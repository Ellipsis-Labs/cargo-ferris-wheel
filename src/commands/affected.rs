//! Ripples command implementation

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use miette::{Result, WrapErr};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};

use crate::analyzer::{CratePathToWorkspaceMap, Dependency, WorkspaceInfo};
use crate::cli::Commands;
use crate::common::FromCommand;
use crate::config::AffectedConfig;
use crate::dependency_filter::DependencyFilter;
use crate::error::FerrisWheelError;

/// JSON output structure for affected analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct AffectedJsonReport {
    pub affected_crates: Vec<AffectedCrate>,
    pub affected_workspaces: Vec<AffectedWorkspace>,
    pub directly_affected_crates: Vec<String>,
    pub directly_affected_workspaces: Vec<AffectedWorkspace>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AffectedWorkspace {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AffectedCrate {
    pub name: String,
    pub workspace: String,
    pub is_directly_affected: bool,
    pub is_standalone: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct CrateId {
    name: String,
    path: PathBuf,
}

impl CrateId {
    fn new(name: String, path: PathBuf) -> Self {
        Self { name, path }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl FromCommand for AffectedConfig {
    fn from_command(command: Commands) -> Result<Self, FerrisWheelError> {
        match command {
            Commands::Ripples {
                files,
                show_crates,
                direct_only,
                exclude_dev,
                exclude_build,
                exclude_target,
                format,
            } => AffectedConfig::builder()
                .with_files(files)
                .with_show_crates(show_crates)
                .with_direct_only(direct_only)
                .with_paths(vec![
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                ])
                .with_format(format.format)
                .with_exclude_dev(exclude_dev)
                .with_exclude_build(exclude_build)
                .with_exclude_target(exclude_target)
                .build(),
            _ => Err(FerrisWheelError::ConfigurationError {
                message: "Invalid command type for AffectedConfig".to_string(),
            }),
        }
    }
}

crate::impl_try_from_command!(AffectedConfig);

/// Execute the ripples command
pub fn execute_affected_command(command: Commands) -> Result<()> {
    let config = AffectedConfig::from_command(command)
        .wrap_err("Failed to parse ripples command configuration")?;

    use crate::executors::CommandExecutor;
    use crate::executors::affected::AffectedExecutor;
    AffectedExecutor::execute(config)
}

/// Analysis of affected crates and workspaces based on changed files
pub struct AffectedAnalysis {
    /// Map from crate identifier to its workspace path
    crate_workspace_index: HashMap<CrateId, PathBuf>,
    /// Map from crate path to crate identifier for quick lookup
    crate_path_index: HashMap<PathBuf, CrateId>,
    /// Map from workspace path to workspace info
    workspaces: HashMap<PathBuf, WorkspaceInfo>,
    /// Crate-level dependency graph keyed by crate identifier
    crate_graph: DiGraph<CrateId, ()>,
    /// Map from crate identifier to node index in the graph
    crate_node_indices: HashMap<CrateId, NodeIndex>,
}

impl AffectedAnalysis {
    pub fn workspaces(&self) -> &HashMap<PathBuf, WorkspaceInfo> {
        &self.workspaces
    }

    pub fn new(
        workspaces: &HashMap<PathBuf, WorkspaceInfo>,
        crate_path_to_workspace: &CratePathToWorkspaceMap,
        filter: DependencyFilter,
    ) -> Result<Self, FerrisWheelError> {
        let mut crate_graph = DiGraph::new();
        let mut crate_node_indices = HashMap::new();
        let mut crate_workspace_index = HashMap::new();
        let mut crate_path_index = HashMap::new();
        let mut crate_ids_by_name: HashMap<String, Vec<CrateId>> = HashMap::new();

        // First pass: create nodes for all crates and build proper mappings
        for (workspace_path, workspace_info) in workspaces {
            let workspace_path = workspace_path.clone();
            for member in workspace_info.members() {
                let crate_path = member
                    .path()
                    .canonicalize()
                    .unwrap_or_else(|_| member.path().clone());
                let crate_id = CrateId::new(member.name().to_string(), crate_path.clone());
                let node_idx = crate_graph.add_node(crate_id.clone());
                crate_node_indices.insert(crate_id.clone(), node_idx);

                crate_workspace_index.insert(
                    crate_id.clone(),
                    crate_path_to_workspace
                        .get(&crate_path)
                        .cloned()
                        .unwrap_or_else(|| workspace_path.clone()),
                );

                crate_path_index.insert(crate_path.clone(), crate_id.clone());

                crate_ids_by_name
                    .entry(crate_id.name().to_string())
                    .or_default()
                    .push(crate_id);
            }
        }

        // Second pass: add edges based on dependencies
        for (workspace_path, workspace_info) in workspaces {
            for member in workspace_info.members() {
                let crate_path = member
                    .path()
                    .canonicalize()
                    .unwrap_or_else(|_| member.path().clone());
                let Some(from_id) = crate_path_index.get(&crate_path).cloned() else {
                    continue;
                };
                let &from_idx = crate_node_indices
                    .get(&from_id)
                    .expect("crate node must exist for analyzed member");

                let mut ctx = DependencyGraphContext {
                    crate_graph: &mut crate_graph,
                    crate_node_indices: &crate_node_indices,
                    crate_ids_by_name: &crate_ids_by_name,
                    crate_path_index: &crate_path_index,
                    workspace_path: workspace_path.as_path(),
                };

                connect_dependencies(member.dependencies(), true, from_idx, &from_id, &mut ctx);

                connect_dependencies(
                    member.dev_dependencies(),
                    filter.include_dev(),
                    from_idx,
                    &from_id,
                    &mut ctx,
                );

                connect_dependencies(
                    member.build_dependencies(),
                    filter.include_build(),
                    from_idx,
                    &from_id,
                    &mut ctx,
                );

                if filter.include_target() {
                    for deps in member.target_dependencies().values() {
                        connect_dependencies(deps, true, from_idx, &from_id, &mut ctx);
                    }
                }
            }
        }

        Ok(Self {
            crate_workspace_index,
            crate_path_index,
            workspaces: workspaces.clone(),
            crate_graph,
            crate_node_indices,
        })
    }

    /// Handle workspace-level Cargo files (Cargo.toml or Cargo.lock)
    fn handle_workspace_cargo_file(
        &self,
        abs_file: &Path,
        cwd: &Path,
        directly_affected_crates: &mut HashSet<CrateId>,
    ) -> bool {
        // Check if this file is at a workspace root
        for ws_path in self.workspaces.keys() {
            let abs_ws_path = if ws_path.is_absolute() {
                ws_path.clone()
            } else {
                cwd.join(ws_path)
            };
            let abs_ws_path = abs_ws_path.canonicalize().unwrap_or(abs_ws_path);

            // Check if the Cargo file is directly in the workspace root
            if let Some(parent) = abs_file.parent()
                && parent == abs_ws_path
            {
                // This is a workspace-level Cargo file
                // Mark all crates in this workspace as directly affected
                for (crate_id, crate_ws_path) in &self.crate_workspace_index {
                    let crate_ws_abs = crate_ws_path
                        .canonicalize()
                        .unwrap_or_else(|_| crate_ws_path.clone());
                    if crate_ws_abs == abs_ws_path {
                        directly_affected_crates.insert(crate_id.clone());
                    }
                }
                return true;
            }
        }
        false
    }

    /// Analyze which crates and workspaces are affected by the given files
    pub fn analyze_affected_files(&self, files: &[String]) -> AffectedResult {
        let mut directly_affected_crates: HashSet<CrateId> = HashSet::new();
        let mut unmatched_files = Vec::new();

        // Get current directory once for efficiency
        let cwd = std::env::current_dir().unwrap_or_default();

        // Map files to crates
        for file in files {
            let file_path = PathBuf::from(file);

            // Normalize the file path to absolute and resolve symlinks
            let abs_file = if file_path.is_absolute() {
                file_path.clone()
            } else {
                cwd.join(&file_path)
            };
            let abs_file = abs_file.canonicalize().unwrap_or(abs_file);

            // Check if this is a Cargo.lock or Cargo.toml file
            let filename = abs_file.file_name().and_then(|f| f.to_str());
            let is_cargo_file = matches!(filename, Some("Cargo.lock") | Some("Cargo.toml"));

            // Handle workspace-level Cargo files
            if is_cargo_file
                && self.handle_workspace_cargo_file(&abs_file, &cwd, &mut directly_affected_crates)
            {
                continue;
            }

            if let Some(crate_id) = self.find_crate_for_file(&abs_file) {
                directly_affected_crates.insert(crate_id);
            } else {
                unmatched_files.push(file.clone());
            }
        }

        // Find all crates affected by reverse dependencies
        let mut all_affected_crates = directly_affected_crates.clone();
        for crate_id in directly_affected_crates.iter() {
            if let Some(&node_idx) = self.crate_node_indices.get(crate_id) {
                self.find_reverse_dependencies(node_idx, &mut all_affected_crates);
            }
        }

        let directly_affected_workspaces: HashSet<String> = directly_affected_crates
            .iter()
            .filter_map(|crate_id| self.workspace_name(crate_id))
            .collect();

        let all_affected_workspaces: HashSet<String> = all_affected_crates
            .iter()
            .filter_map(|crate_id| self.workspace_name(crate_id))
            .collect();

        AffectedResult {
            directly_affected_crates,
            all_affected_crates,
            directly_affected_workspaces,
            all_affected_workspaces,
            unmatched_files,
        }
    }

    fn find_reverse_dependencies(&self, node_idx: NodeIndex, affected: &mut HashSet<CrateId>) {
        use petgraph::Direction;

        for edge in self
            .crate_graph
            .edges_directed(node_idx, Direction::Incoming)
        {
            let source_idx = edge.source();
            let source_crate = self.crate_graph[source_idx].clone();
            if affected.insert(source_crate.clone()) {
                // Recursively find more reverse dependencies
                self.find_reverse_dependencies(source_idx, affected);
            }
        }
    }

    fn find_crate_for_file(&self, abs_file: &Path) -> Option<CrateId> {
        let canonical = abs_file
            .canonicalize()
            .unwrap_or_else(|_| abs_file.to_path_buf());

        let mut best_match: Option<(usize, CrateId)> = None;

        for (crate_path, crate_id) in &self.crate_path_index {
            let match_path = (canonical.starts_with(crate_path)
                || abs_file.starts_with(crate_path))
            .then_some(crate_path);

            if let Some(path) = match_path {
                let match_len = path.as_os_str().len();
                match &best_match {
                    None => best_match = Some((match_len, crate_id.clone())),
                    Some((best_len, _)) if match_len > *best_len => {
                        best_match = Some((match_len, crate_id.clone()))
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(_, id)| id)
    }

    pub(crate) fn workspace_name(&self, crate_id: &CrateId) -> Option<String> {
        self.crate_workspace_index
            .get(crate_id)
            .and_then(|ws_path| self.workspaces.get(ws_path))
            .map(|ws| ws.name().to_string())
    }
}

struct DependencyGraphContext<'a> {
    crate_graph: &'a mut DiGraph<CrateId, ()>,
    crate_node_indices: &'a HashMap<CrateId, NodeIndex>,
    crate_ids_by_name: &'a HashMap<String, Vec<CrateId>>,
    crate_path_index: &'a HashMap<PathBuf, CrateId>,
    workspace_path: &'a Path,
}

fn connect_dependencies(
    deps: &[Dependency],
    include: bool,
    from_idx: NodeIndex,
    from_id: &CrateId,
    ctx: &mut DependencyGraphContext<'_>,
) {
    if !include {
        return;
    }

    for dep in deps {
        if let Some(to_idx) = resolve_dependency_crate_id(
            dep,
            from_id,
            ctx.workspace_path,
            ctx.crate_ids_by_name,
            ctx.crate_path_index,
        )
        .and_then(|target_id| ctx.crate_node_indices.get(&target_id).copied())
        {
            ctx.crate_graph.add_edge(from_idx, to_idx, ());
        }
    }
}

fn resolve_dependency_crate_id(
    dep: &Dependency,
    from_id: &CrateId,
    workspace_path: &Path,
    crate_ids_by_name: &HashMap<String, Vec<CrateId>>,
    crate_path_index: &HashMap<PathBuf, CrateId>,
) -> Option<CrateId> {
    if let Some(dep_path) = dep.path() {
        let base = if dep.is_workspace() {
            workspace_path
        } else {
            from_id.path()
        };

        let absolute = if dep_path.is_absolute() {
            dep_path.clone()
        } else {
            base.join(dep_path)
        };

        let canonical = absolute.canonicalize().unwrap_or_else(|_| absolute.clone());

        crate_path_index
            .get(&canonical)
            .or_else(|| crate_path_index.get(&absolute))
            .cloned()
            .or_else(|| {
                crate_path_index
                    .iter()
                    .find_map(|(candidate_path, candidate_id)| {
                        if canonical.starts_with(candidate_path)
                            || candidate_path.starts_with(&canonical)
                        {
                            Some(candidate_id.clone())
                        } else {
                            None
                        }
                    })
            })
    } else {
        crate_ids_by_name.get(dep.name()).and_then(|ids| {
            if ids.len() == 1 {
                Some(ids[0].clone())
            } else {
                None
            }
        })
    }
}

pub struct AffectedResult {
    pub(crate) directly_affected_crates: HashSet<CrateId>,
    pub(crate) all_affected_crates: HashSet<CrateId>,
    pub(crate) directly_affected_workspaces: HashSet<String>,
    pub(crate) all_affected_workspaces: HashSet<String>,
    pub(crate) unmatched_files: Vec<String>,
}

impl AffectedResult {
    pub fn to_json_report(&self, analysis: &AffectedAnalysis) -> AffectedJsonReport {
        let mut affected_crates = Vec::new();

        for crate_id in &self.all_affected_crates {
            let workspace_info = analysis
                .crate_workspace_index
                .get(crate_id)
                .and_then(|ws_path| analysis.workspaces.get(ws_path));

            let (workspace_name, is_standalone) = workspace_info
                .map(|ws| (ws.name().to_string(), ws.is_standalone()))
                .unwrap_or_else(|| ("unknown".to_string(), false));

            affected_crates.push(AffectedCrate {
                name: crate_id.name().to_string(),
                workspace: workspace_name,
                is_directly_affected: self.directly_affected_crates.contains(crate_id),
                is_standalone,
            });
        }

        // Sort affected crates by name for deterministic output
        affected_crates.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.workspace.cmp(&b.workspace))
        });

        // Create affected workspace objects with paths
        let mut affected_workspaces: Vec<AffectedWorkspace> = self
            .all_affected_workspaces
            .iter()
            .map(|ws_name| {
                // Find the workspace path by looking through all workspaces
                let ws_path = analysis
                    .workspaces
                    .iter()
                    .find(|(_, ws_info)| ws_info.name() == ws_name)
                    .map(|(path, _)| path.display().to_string())
                    .unwrap_or_else(|| "(unknown)".to_string());

                AffectedWorkspace {
                    name: ws_name.clone(),
                    path: ws_path,
                }
            })
            .collect();
        affected_workspaces.sort_by(|a, b| a.name.cmp(&b.name));

        let mut directly_affected_crates: Vec<String> = self
            .directly_affected_crates
            .iter()
            .map(|crate_id| crate_id.name().to_string())
            .collect();
        directly_affected_crates.sort();
        directly_affected_crates.dedup();

        let mut directly_affected_workspaces: Vec<AffectedWorkspace> = self
            .directly_affected_workspaces
            .iter()
            .map(|ws_name| {
                // Find the workspace path by looking through all workspaces
                let ws_path = analysis
                    .workspaces
                    .iter()
                    .find(|(_, ws_info)| ws_info.name() == ws_name)
                    .map(|(path, _)| path.display().to_string())
                    .unwrap_or_else(|| "(unknown)".to_string());

                AffectedWorkspace {
                    name: ws_name.clone(),
                    path: ws_path,
                }
            })
            .collect();
        directly_affected_workspaces.sort_by(|a, b| a.name.cmp(&b.name));

        AffectedJsonReport {
            affected_crates,
            affected_workspaces,
            directly_affected_crates,
            directly_affected_workspaces,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;

    fn contains_crate(crates: &HashSet<CrateId>, name: &str) -> bool {
        crates.iter().any(|id| id.name() == name)
    }

    fn count_crate(crates: &HashSet<CrateId>, name: &str) -> usize {
        crates.iter().filter(|id| id.name() == name).count()
    }

    fn create_test_workspace_with_duplicates() -> TempDir {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Create workspace A with phoenix-v2-api
        fs::create_dir_all(root.join("workspace-a")).unwrap();
        fs::write(
            root.join("workspace-a/Cargo.toml"),
            r#"
[workspace]
members = ["phoenix-v2-api", "other-crate"]

[workspace.dependencies]
serde = "1.0"
"#,
        )
        .unwrap();

        // Create phoenix-v2-api in workspace A
        fs::create_dir_all(root.join("workspace-a/phoenix-v2-api/src")).unwrap();
        fs::write(
            root.join("workspace-a/phoenix-v2-api/Cargo.toml"),
            r#"
[package]
name = "phoenix-v2-api"
"#,
        )
        .unwrap();
        fs::write(
            root.join("workspace-a/phoenix-v2-api/src/lib.rs"),
            "pub fn api_a() {}",
        )
        .unwrap();

        // Create other-crate that depends on phoenix-v2-api
        fs::create_dir_all(root.join("workspace-a/other-crate/src")).unwrap();
        fs::write(
            root.join("workspace-a/other-crate/Cargo.toml"),
            r#"
[package]
name = "other-crate"

[dependencies]
phoenix-v2-api = { path = "../phoenix-v2-api" }
"#,
        )
        .unwrap();
        fs::write(
            root.join("workspace-a/other-crate/src/lib.rs"),
            "pub fn other() {}",
        )
        .unwrap();

        // Create workspace B with its own phoenix-v2-api
        fs::create_dir_all(root.join("workspace-b")).unwrap();
        fs::write(
            root.join("workspace-b/Cargo.toml"),
            r#"
[workspace]
members = ["phoenix-v2-api", "consumer-crate"]
"#,
        )
        .unwrap();

        // Create phoenix-v2-api in workspace B
        fs::create_dir_all(root.join("workspace-b/phoenix-v2-api/src")).unwrap();
        fs::write(
            root.join("workspace-b/phoenix-v2-api/Cargo.toml"),
            r#"
[package]
name = "phoenix-v2-api"
"#,
        )
        .unwrap();
        fs::write(
            root.join("workspace-b/phoenix-v2-api/src/main.rs"),
            "fn main() { println!(\"API B\"); }",
        )
        .unwrap();

        // Create consumer-crate that depends on this phoenix-v2-api
        fs::create_dir_all(root.join("workspace-b/consumer-crate/src")).unwrap();
        fs::write(
            root.join("workspace-b/consumer-crate/Cargo.toml"),
            r#"
[package]
name = "consumer-crate"

[dependencies]
phoenix-v2-api = { path = "../phoenix-v2-api" }
"#,
        )
        .unwrap();
        fs::write(
            root.join("workspace-b/consumer-crate/src/lib.rs"),
            "pub fn consumer() {}",
        )
        .unwrap();

        temp
    }

    fn create_simple_test_workspace() -> TempDir {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Create a simple workspace
        fs::create_dir_all(root.join("my-workspace")).unwrap();
        fs::write(
            root.join("my-workspace/Cargo.toml"),
            r#"
[workspace]
members = ["crate-a", "crate-b"]
"#,
        )
        .unwrap();

        // Create crate-a
        fs::create_dir_all(root.join("my-workspace/crate-a/src")).unwrap();
        fs::write(
            root.join("my-workspace/crate-a/Cargo.toml"),
            r#"
[package]
name = "crate-a"

[dependencies]
crate-b = { path = "../crate-b" }
"#,
        )
        .unwrap();
        fs::write(
            root.join("my-workspace/crate-a/src/lib.rs"),
            "pub fn function_a() {}",
        )
        .unwrap();
        fs::write(
            root.join("my-workspace/crate-a/src/main.rs"),
            "fn main() {}",
        )
        .unwrap();

        // Create crate-b
        fs::create_dir_all(root.join("my-workspace/crate-b/src")).unwrap();
        fs::write(
            root.join("my-workspace/crate-b/Cargo.toml"),
            r#"
[package]
name = "crate-b"
"#,
        )
        .unwrap();
        fs::write(
            root.join("my-workspace/crate-b/src/lib.rs"),
            "pub fn function_b() {}",
        )
        .unwrap();

        // Add Cargo.lock file to the workspace root
        fs::write(
            root.join("my-workspace/Cargo.lock"),
            r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "crate-a"
version = "0.1.0"

[[package]]
name = "crate-b"
version = "0.1.0"
"#,
        )
        .unwrap();

        temp
    }

    fn build_test_analysis(workspace_root: &Path) -> AffectedAnalysis {
        use crate::analyzer::WorkspaceAnalyzer;

        let mut analyzer = WorkspaceAnalyzer::new();
        analyzer
            .discover_workspaces(&[workspace_root.to_path_buf()], None)
            .unwrap();

        AffectedAnalysis::new(
            analyzer.workspaces(),
            analyzer.crate_path_to_workspace(),
            crate::dependency_filter::DependencyFilter::default(),
        )
        .unwrap()
    }

    #[test]
    fn test_file_to_crate_mapping_with_duplicates() {
        let temp = create_test_workspace_with_duplicates();
        let analysis = build_test_analysis(temp.path());

        // Test that files map to the correct crate based on longest path match
        let files_a = vec![format!(
            "{}/workspace-a/phoenix-v2-api/src/lib.rs",
            temp.path().display()
        )];
        let result_a = analysis.analyze_affected_files(&files_a);

        assert!(contains_crate(
            &result_a.directly_affected_crates,
            "phoenix-v2-api"
        ));
        assert_eq!(
            count_crate(&result_a.directly_affected_crates, "phoenix-v2-api"),
            1
        );
        assert!(
            result_a
                .directly_affected_workspaces
                .contains("workspace-a")
        );
        assert!(
            !result_a
                .directly_affected_workspaces
                .contains("workspace-b")
        );

        // Test workspace B's phoenix-v2-api
        let files_b = vec![format!(
            "{}/workspace-b/phoenix-v2-api/src/main.rs",
            temp.path().display()
        )];
        let result_b = analysis.analyze_affected_files(&files_b);

        assert!(contains_crate(
            &result_b.directly_affected_crates,
            "phoenix-v2-api"
        ));
        assert_eq!(
            count_crate(&result_b.directly_affected_crates, "phoenix-v2-api"),
            1
        );
        assert!(
            result_b
                .directly_affected_workspaces
                .contains("workspace-b")
        );
        assert!(
            !result_b
                .directly_affected_workspaces
                .contains("workspace-a")
        );
    }

    #[test]
    fn test_duplicate_crate_names_multiple_changes() {
        let temp = create_test_workspace_with_duplicates();
        let analysis = build_test_analysis(temp.path());

        let files = vec![
            format!(
                "{}/workspace-a/phoenix-v2-api/src/lib.rs",
                temp.path().display()
            ),
            format!(
                "{}/workspace-b/phoenix-v2-api/src/main.rs",
                temp.path().display()
            ),
        ];

        let result = analysis.analyze_affected_files(&files);

        assert_eq!(
            count_crate(&result.directly_affected_crates, "phoenix-v2-api"),
            2
        );
        assert!(result.directly_affected_workspaces.contains("workspace-a"));
        assert!(result.directly_affected_workspaces.contains("workspace-b"));
    }

    #[test]
    fn test_reverse_dependencies() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Modify crate-b
        let files = vec![format!(
            "{}/my-workspace/crate-b/src/lib.rs",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);

        // crate-b should be directly affected
        assert!(contains_crate(&result.directly_affected_crates, "crate-b"));
        assert_eq!(result.directly_affected_crates.len(), 1);

        // crate-a should be affected via reverse dependency
        assert!(contains_crate(&result.all_affected_crates, "crate-a"));
        assert!(contains_crate(&result.all_affected_crates, "crate-b"));
        assert_eq!(result.all_affected_crates.len(), 2);
    }

    #[test]
    fn test_unmatched_files() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Test with files that don't belong to any crate
        let files = vec![
            "/tmp/some-random-file.rs".to_string(),
            format!("{}/README.md", temp.path().display()),
        ];
        let result = analysis.analyze_affected_files(&files);

        assert_eq!(result.unmatched_files.len(), 2);
        assert!(result.directly_affected_crates.is_empty());
        assert!(result.directly_affected_workspaces.is_empty());
    }

    #[test]
    fn test_relative_paths() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Change to the workspace directory to test relative paths
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        // Use relative paths
        let files = vec!["my-workspace/crate-a/src/lib.rs".to_string()];
        let result = analysis.analyze_affected_files(&files);

        assert!(contains_crate(&result.directly_affected_crates, "crate-a"));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_json_report_generation() {
        let temp = create_test_workspace_with_duplicates();
        let analysis = build_test_analysis(temp.path());

        let files = vec![
            format!(
                "{}/workspace-a/phoenix-v2-api/src/lib.rs",
                temp.path().display()
            ),
            format!(
                "{}/workspace-a/other-crate/src/lib.rs",
                temp.path().display()
            ),
        ];
        let result = analysis.analyze_affected_files(&files);

        let json_report = result.to_json_report(&analysis);

        // Check that all directly affected crates are marked correctly
        for crate_info in &json_report.affected_crates {
            if crate_info.name == "phoenix-v2-api" || crate_info.name == "other-crate" {
                assert!(crate_info.is_directly_affected);
            }
        }

        // Check workspace information
        assert!(
            json_report
                .directly_affected_workspaces
                .iter()
                .any(|ws| ws.name == "workspace-a")
        );
    }

    #[test]
    fn test_multiple_files_same_crate() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Multiple files from the same crate
        let files = vec![
            format!("{}/my-workspace/crate-a/src/lib.rs", temp.path().display()),
            format!("{}/my-workspace/crate-a/src/main.rs", temp.path().display()),
        ];
        let result = analysis.analyze_affected_files(&files);

        // Should only count crate-a once
        assert_eq!(result.directly_affected_crates.len(), 1);
        assert!(contains_crate(&result.directly_affected_crates, "crate-a"));
    }

    #[test]
    fn test_cross_workspace_dependencies() {
        let temp = create_test_workspace_with_duplicates();
        let analysis = build_test_analysis(temp.path());

        // Modify consumer-crate in workspace B
        let files = vec![format!(
            "{}/workspace-b/consumer-crate/src/lib.rs",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);

        // Only consumer-crate should be directly affected
        assert!(contains_crate(
            &result.directly_affected_crates,
            "consumer-crate"
        ));

        // Workspace B should be affected
        assert!(result.directly_affected_workspaces.contains("workspace-b"));

        // Workspace A should NOT be affected (different phoenix-v2-api)
        assert!(!result.all_affected_workspaces.contains("workspace-a"));
    }

    fn create_mixed_workspace_and_standalone() -> TempDir {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Create a real workspace
        fs::create_dir_all(root.join("real-workspace")).unwrap();
        fs::write(
            root.join("real-workspace/Cargo.toml"),
            r#"
[workspace]
members = ["crate-a", "crate-b"]

[workspace.dependencies]
serde = "1.0"
"#,
        )
        .unwrap();

        // Create workspace members
        fs::create_dir_all(root.join("real-workspace/crate-a/src")).unwrap();
        fs::write(
            root.join("real-workspace/crate-a/Cargo.toml"),
            r#"
[package]
name = "crate-a"
version = "0.1.0"

[dependencies]
crate-b = { path = "../crate-b" }
"#,
        )
        .unwrap();
        fs::write(
            root.join("real-workspace/crate-a/src/lib.rs"),
            "pub fn func_a() {}",
        )
        .unwrap();

        fs::create_dir_all(root.join("real-workspace/crate-b/src")).unwrap();
        fs::write(
            root.join("real-workspace/crate-b/Cargo.toml"),
            r#"
[package]
name = "crate-b"
version = "0.1.0"
"#,
        )
        .unwrap();
        fs::write(
            root.join("real-workspace/crate-b/src/lib.rs"),
            "pub fn func_b() {}",
        )
        .unwrap();

        // Add Cargo.lock file to the workspace root
        fs::write(
            root.join("real-workspace/Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual \
             editing.\nversion = 3\n\n[[package]]\nname = \"crate-a\"\nversion = \
             \"0.1.0\"\n\n[[package]]\nname = \"crate-b\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        // Create a standalone crate (like test programs)
        fs::create_dir_all(root.join("standalone-test-crate/src")).unwrap();
        fs::write(
            root.join("standalone-test-crate/Cargo.toml"),
            r#"
[package]
name = "standalone-test-crate"
version = "0.1.0"

[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
serde = "1.0"
"#,
        )
        .unwrap();
        fs::write(
            root.join("standalone-test-crate/src/lib.rs"),
            "pub fn test_func() {}",
        )
        .unwrap();
        // Add Cargo.lock file so it's detected as a standalone crate
        fs::write(
            root.join("standalone-test-crate/Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual \
             editing.\nversion = 3\n\n[[package]]\nname = \"standalone-test-crate\"\nversion = \
             \"0.1.0\"",
        )
        .unwrap();

        // Create another standalone crate
        fs::create_dir_all(root.join("another-standalone/src")).unwrap();
        fs::write(
            root.join("another-standalone/Cargo.toml"),
            r#"
[package]
name = "another-standalone"
version = "0.1.0"
"#,
        )
        .unwrap();
        fs::write(
            root.join("another-standalone/src/lib.rs"),
            "pub fn another_func() {}",
        )
        .unwrap();
        // Add Cargo.lock file so it's detected as a standalone crate
        fs::write(
            root.join("another-standalone/Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual \
             editing.\nversion = 3\n\n[[package]]\nname = \"another-standalone\"\nversion = \
             \"0.1.0\"",
        )
        .unwrap();

        temp
    }

    #[test]
    fn test_standalone_crate_detection_in_json_report() {
        let temp = create_mixed_workspace_and_standalone();
        let analysis = build_test_analysis(temp.path());

        // Test affecting a standalone crate
        let files = vec![format!(
            "{}/standalone-test-crate/src/lib.rs",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);
        let json_report = result.to_json_report(&analysis);

        // Should have one affected crate
        assert_eq!(json_report.affected_crates.len(), 1);

        let standalone_crate = &json_report.affected_crates[0];
        assert_eq!(standalone_crate.name, "standalone-test-crate");
        assert_eq!(standalone_crate.workspace, "standalone-test-crate");
        assert!(standalone_crate.is_directly_affected);
        assert!(standalone_crate.is_standalone); // This is the key test!
    }

    #[test]
    fn test_workspace_crate_detection_in_json_report() {
        let temp = create_mixed_workspace_and_standalone();
        let analysis = build_test_analysis(temp.path());

        // Test affecting a workspace member
        let files = vec![format!(
            "{}/real-workspace/crate-a/src/lib.rs",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);
        let json_report = result.to_json_report(&analysis);

        // Should have two affected crates (crate-a and crate-b due to reverse deps)
        assert!(!json_report.affected_crates.is_empty());

        let crate_a = json_report
            .affected_crates
            .iter()
            .find(|c| c.name == "crate-a")
            .unwrap();

        assert_eq!(crate_a.name, "crate-a");
        assert_eq!(crate_a.workspace, "real-workspace");
        assert!(crate_a.is_directly_affected);
        assert!(!crate_a.is_standalone); // This is the key test!
    }

    #[test]
    fn test_mixed_standalone_and_workspace_detection() {
        let temp = create_mixed_workspace_and_standalone();
        let analysis = build_test_analysis(temp.path());

        // Test affecting both standalone and workspace crates
        let files = vec![
            format!("{}/standalone-test-crate/src/lib.rs", temp.path().display()),
            format!(
                "{}/real-workspace/crate-a/src/lib.rs",
                temp.path().display()
            ),
            format!("{}/another-standalone/src/lib.rs", temp.path().display()),
        ];
        let result = analysis.analyze_affected_files(&files);
        let json_report = result.to_json_report(&analysis);

        // Should have multiple affected crates
        assert!(json_report.affected_crates.len() >= 3);

        // Check standalone crates
        let standalone_crates: Vec<_> = json_report
            .affected_crates
            .iter()
            .filter(|c| c.is_standalone)
            .collect();

        assert_eq!(standalone_crates.len(), 2);

        let standalone_names: Vec<&str> =
            standalone_crates.iter().map(|c| c.name.as_str()).collect();

        assert!(standalone_names.contains(&"standalone-test-crate"));
        assert!(standalone_names.contains(&"another-standalone"));

        // Check workspace crates
        let workspace_crates: Vec<_> = json_report
            .affected_crates
            .iter()
            .filter(|c| !c.is_standalone)
            .collect();

        assert!(!workspace_crates.is_empty());

        let workspace_names: Vec<&str> = workspace_crates.iter().map(|c| c.name.as_str()).collect();

        assert!(workspace_names.contains(&"crate-a"));
    }

    #[test]
    fn test_standalone_crate_workspace_name_equals_crate_name() {
        let temp = create_mixed_workspace_and_standalone();
        let analysis = build_test_analysis(temp.path());

        let files = vec![format!(
            "{}/standalone-test-crate/src/lib.rs",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);
        let json_report = result.to_json_report(&analysis);

        let standalone_crate = json_report
            .affected_crates
            .iter()
            .find(|c| c.is_standalone)
            .unwrap();

        // For standalone crates, workspace name should equal crate name
        assert_eq!(standalone_crate.name, standalone_crate.workspace);
        assert_eq!(standalone_crate.name, "standalone-test-crate");
    }

    #[test]
    fn test_workspace_crate_workspace_name_differs_from_crate_name() {
        let temp = create_mixed_workspace_and_standalone();
        let analysis = build_test_analysis(temp.path());

        let files = vec![format!(
            "{}/real-workspace/crate-a/src/lib.rs",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);
        let json_report = result.to_json_report(&analysis);

        let workspace_crate = json_report
            .affected_crates
            .iter()
            .find(|c| !c.is_standalone && c.name == "crate-a")
            .unwrap();

        // For workspace members, workspace name should differ from crate name
        assert_ne!(workspace_crate.name, workspace_crate.workspace);
        assert_eq!(workspace_crate.name, "crate-a");
        assert_eq!(workspace_crate.workspace, "real-workspace");
    }

    #[test]
    fn test_json_report_serialization_with_standalone_field() {
        let temp = create_mixed_workspace_and_standalone();
        let analysis = build_test_analysis(temp.path());

        let files = vec![format!(
            "{}/standalone-test-crate/src/lib.rs",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);
        let json_report = result.to_json_report(&analysis);

        // Test that the JSON report can be serialized and includes the is_standalone
        // field
        let json_str = serde_json::to_string(&json_report).unwrap();

        // Verify the JSON contains the is_standalone field
        assert!(json_str.contains("is_standalone"));
        assert!(json_str.contains("\"is_standalone\":true"));

        // Verify we can deserialize it back
        let parsed: AffectedJsonReport = serde_json::from_str(&json_str).unwrap();
        assert_eq!(
            parsed.affected_crates.len(),
            json_report.affected_crates.len()
        );

        let standalone_crate = &parsed.affected_crates[0];
        assert!(standalone_crate.is_standalone);
    }

    #[test]
    fn test_cargo_manifest_files_mapping() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Test that Cargo.toml files are properly mapped to their crates
        let files = vec![
            format!("{}/my-workspace/crate-a/Cargo.toml", temp.path().display()),
            format!("{}/my-workspace/crate-b/Cargo.toml", temp.path().display()),
        ];
        let result = analysis.analyze_affected_files(&files);

        // Both crates should be directly affected by their Cargo.toml files
        assert_eq!(result.directly_affected_crates.len(), 2);
        assert!(contains_crate(&result.directly_affected_crates, "crate-a"));
        assert!(contains_crate(&result.directly_affected_crates, "crate-b"));

        // The workspace should be affected
        assert!(result.directly_affected_workspaces.contains("my-workspace"));
    }

    #[test]
    fn test_workspace_cargo_toml_mapping() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Test workspace-level Cargo.toml
        let files = vec![format!("{}/my-workspace/Cargo.toml", temp.path().display())];
        let result = analysis.analyze_affected_files(&files);

        // Workspace Cargo.toml should affect all workspace members
        assert_eq!(result.directly_affected_crates.len(), 2);
        assert!(contains_crate(&result.directly_affected_crates, "crate-a"));
        assert!(contains_crate(&result.directly_affected_crates, "crate-b"));
        assert!(result.directly_affected_workspaces.contains("my-workspace"));
        assert!(result.unmatched_files.is_empty());
    }

    #[test]
    fn test_cargo_lock_file_mapping() {
        let temp = create_mixed_workspace_and_standalone();
        let analysis = build_test_analysis(temp.path());

        // Test Cargo.lock files
        let files = vec![
            format!("{}/real-workspace/Cargo.lock", temp.path().display()),
            format!("{}/standalone-test-crate/Cargo.lock", temp.path().display()),
        ];
        let result = analysis.analyze_affected_files(&files);

        // Standalone crate's Cargo.lock should map to the crate
        assert!(contains_crate(
            &result.directly_affected_crates,
            "standalone-test-crate",
        ));

        // Workspace Cargo.lock should affect all workspace members
        assert!(contains_crate(&result.directly_affected_crates, "crate-a"));
        assert!(contains_crate(&result.directly_affected_crates, "crate-b"));

        // No unmatched files
        assert!(result.unmatched_files.is_empty());
    }

    #[test]
    fn test_workspace_cargo_lock_affects_all_members() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Test that changing Cargo.lock at workspace root affects all members
        let files = vec![format!("{}/my-workspace/Cargo.lock", temp.path().display())];
        let result = analysis.analyze_affected_files(&files);

        // All workspace members should be directly affected
        assert!(contains_crate(&result.directly_affected_crates, "crate-a"));
        assert!(contains_crate(&result.directly_affected_crates, "crate-b"));
        assert_eq!(result.directly_affected_crates.len(), 2);

        // The workspace should be affected
        assert!(result.directly_affected_workspaces.contains("my-workspace"));

        // No unmatched files
        assert!(result.unmatched_files.is_empty());
    }

    #[test]
    fn test_workspace_cargo_toml_affects_all_members() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Test that changing workspace Cargo.toml affects all members
        let files = vec![format!("{}/my-workspace/Cargo.toml", temp.path().display())];
        let result = analysis.analyze_affected_files(&files);

        // All workspace members should be directly affected
        assert!(contains_crate(&result.directly_affected_crates, "crate-a"));
        assert!(contains_crate(&result.directly_affected_crates, "crate-b"));
        assert_eq!(result.directly_affected_crates.len(), 2);

        // The workspace should be affected
        assert!(result.directly_affected_workspaces.contains("my-workspace"));

        // No unmatched files
        assert!(result.unmatched_files.is_empty());
    }

    #[test]
    fn test_standalone_cargo_lock_affects_only_that_crate() {
        let temp = create_mixed_workspace_and_standalone();
        let analysis = build_test_analysis(temp.path());

        // Test that changing a standalone crate's Cargo.lock only affects that crate
        let files = vec![format!(
            "{}/standalone-test-crate/Cargo.lock",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);

        // Only the standalone crate should be affected
        assert!(contains_crate(
            &result.directly_affected_crates,
            "standalone-test-crate",
        ));
        assert_eq!(result.directly_affected_crates.len(), 1);

        // No workspace members should be affected
        assert!(!contains_crate(&result.directly_affected_crates, "crate-a"));
        assert!(!contains_crate(&result.directly_affected_crates, "crate-b"));

        // No unmatched files
        assert!(result.unmatched_files.is_empty());
    }

    #[test]
    fn test_crate_cargo_toml_affects_dependents() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Test that changing a crate's Cargo.toml affects its dependents
        let files = vec![format!(
            "{}/my-workspace/crate-b/Cargo.toml",
            temp.path().display()
        )];
        let result = analysis.analyze_affected_files(&files);

        // crate-b should be directly affected
        assert!(contains_crate(&result.directly_affected_crates, "crate-b"));
        assert_eq!(result.directly_affected_crates.len(), 1);

        // crate-a depends on crate-b, so it should be affected through reverse
        // dependencies
        assert!(contains_crate(&result.all_affected_crates, "crate-a"));
        assert!(contains_crate(&result.all_affected_crates, "crate-b"));
        assert_eq!(result.all_affected_crates.len(), 2);
    }

    #[test]
    fn test_multiple_cargo_files_affected() {
        let temp = create_simple_test_workspace();
        let analysis = build_test_analysis(temp.path());

        // Test multiple Cargo files changed at once
        let files = vec![
            format!("{}/my-workspace/Cargo.toml", temp.path().display()),
            format!("{}/my-workspace/Cargo.lock", temp.path().display()),
            format!("{}/my-workspace/crate-a/Cargo.toml", temp.path().display()),
        ];
        let result = analysis.analyze_affected_files(&files);

        // All crates should be directly affected
        assert!(contains_crate(&result.directly_affected_crates, "crate-a"));
        assert!(contains_crate(&result.directly_affected_crates, "crate-b"));
        assert_eq!(result.directly_affected_crates.len(), 2);

        // No unmatched files
        assert!(result.unmatched_files.is_empty());
    }

    #[test]
    fn test_nested_workspace_cargo_lock() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Create a nested workspace structure
        fs::create_dir_all(root.join("outer-workspace")).unwrap();
        fs::write(
            root.join("outer-workspace/Cargo.toml"),
            r#"
[workspace]
members = ["inner-workspace", "outer-crate"]
"#,
        )
        .unwrap();

        // Create inner workspace
        fs::create_dir_all(root.join("outer-workspace/inner-workspace")).unwrap();
        fs::write(
            root.join("outer-workspace/inner-workspace/Cargo.toml"),
            r#"
[workspace]
members = ["inner-crate-a", "inner-crate-b"]
"#,
        )
        .unwrap();

        // Create crates
        fs::create_dir_all(root.join("outer-workspace/inner-workspace/inner-crate-a/src")).unwrap();
        fs::write(
            root.join("outer-workspace/inner-workspace/inner-crate-a/Cargo.toml"),
            r#"
[package]
name = "inner-crate-a"
"#,
        )
        .unwrap();
        fs::write(
            root.join("outer-workspace/inner-workspace/inner-crate-a/src/lib.rs"),
            "pub fn func() {}",
        )
        .unwrap();

        fs::create_dir_all(root.join("outer-workspace/inner-workspace/inner-crate-b/src")).unwrap();
        fs::write(
            root.join("outer-workspace/inner-workspace/inner-crate-b/Cargo.toml"),
            r#"
[package]
name = "inner-crate-b"
"#,
        )
        .unwrap();
        fs::write(
            root.join("outer-workspace/inner-workspace/inner-crate-b/src/lib.rs"),
            "pub fn func() {}",
        )
        .unwrap();

        fs::create_dir_all(root.join("outer-workspace/outer-crate/src")).unwrap();
        fs::write(
            root.join("outer-workspace/outer-crate/Cargo.toml"),
            r#"
[package]
name = "outer-crate"
"#,
        )
        .unwrap();
        fs::write(
            root.join("outer-workspace/outer-crate/src/lib.rs"),
            "pub fn func() {}",
        )
        .unwrap();

        // Add Cargo.lock files
        fs::write(
            root.join("outer-workspace/Cargo.lock"),
            "# Outer workspace lock file",
        )
        .unwrap();
        fs::write(
            root.join("outer-workspace/inner-workspace/Cargo.lock"),
            "# Inner workspace lock file",
        )
        .unwrap();

        let analysis = build_test_analysis(root);

        // Test that inner workspace Cargo.lock only affects inner crates
        let files = vec![format!(
            "{}/outer-workspace/inner-workspace/Cargo.lock",
            root.display()
        )];
        let result = analysis.analyze_affected_files(&files);

        assert!(contains_crate(
            &result.directly_affected_crates,
            "inner-crate-a"
        ));
        assert!(contains_crate(
            &result.directly_affected_crates,
            "inner-crate-b"
        ));
        assert!(!contains_crate(
            &result.directly_affected_crates,
            "outer-crate"
        ));
        assert_eq!(result.directly_affected_crates.len(), 2);
    }
}
