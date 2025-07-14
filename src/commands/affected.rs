//! Ripples command implementation

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use miette::{Result, WrapErr};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};

use crate::analyzer::WorkspaceInfo;
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
    /// Map from (crate_name, crate_path) to workspace path
    crate_path_to_workspace: HashMap<(String, PathBuf), PathBuf>,
    /// Map from crate name to crate paths (can have multiple paths per crate
    /// name)
    crate_to_paths: HashMap<String, Vec<PathBuf>>,
    /// Map from workspace path to workspace info
    workspaces: HashMap<PathBuf, WorkspaceInfo>,
    /// Crate-level dependency graph
    crate_graph: DiGraph<String, ()>,
    /// Map from crate name to node index in the graph
    crate_node_indices: HashMap<String, NodeIndex>,
}

impl AffectedAnalysis {
    pub fn workspaces(&self) -> &HashMap<PathBuf, WorkspaceInfo> {
        &self.workspaces
    }

    pub fn new(
        workspaces: &HashMap<PathBuf, WorkspaceInfo>,
        _crate_to_workspace: &HashMap<String, PathBuf>,
        crate_to_paths: &HashMap<String, Vec<PathBuf>>,
        filter: DependencyFilter,
    ) -> Result<Self, FerrisWheelError> {
        let mut crate_graph = DiGraph::new();
        let mut crate_node_indices = HashMap::new();
        let mut crate_path_to_workspace = HashMap::new();

        // First pass: create nodes for all crates and build proper mappings
        for (workspace_path, workspace_info) in workspaces {
            for member in workspace_info.members() {
                // Add node to graph
                let node_idx = crate_graph.add_node(member.name().to_string());
                crate_node_indices.insert(member.name().to_string(), node_idx);

                // Map (crate_name, crate_path) to workspace
                crate_path_to_workspace.insert(
                    (member.name().to_string(), member.path().clone()),
                    workspace_path.clone(),
                );
            }
        }

        // Second pass: add edges based on dependencies
        for workspace_info in workspaces.values() {
            for member in workspace_info.members() {
                if let Some(&from_idx) = crate_node_indices.get(member.name()) {
                    // Add edges for all dependency types
                    for dep in member.dependencies() {
                        if let Some(&to_idx) = crate_node_indices.get(dep.name()) {
                            crate_graph.add_edge(from_idx, to_idx, ());
                        }
                    }

                    // Include dev dependencies unless excluded
                    if filter.include_dev() {
                        for dep in member.dev_dependencies() {
                            if let Some(&to_idx) = crate_node_indices.get(dep.name()) {
                                crate_graph.add_edge(from_idx, to_idx, ());
                            }
                        }
                    }

                    // Include build dependencies unless excluded
                    if filter.include_build() {
                        for dep in member.build_dependencies() {
                            if let Some(&to_idx) = crate_node_indices.get(dep.name()) {
                                crate_graph.add_edge(from_idx, to_idx, ());
                            }
                        }
                    }

                    // Include target-specific dependencies unless excluded
                    if filter.include_target() {
                        for target_deps in member.target_dependencies().values() {
                            for dep in target_deps {
                                if let Some(&to_idx) = crate_node_indices.get(dep.name()) {
                                    crate_graph.add_edge(from_idx, to_idx, ());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            crate_path_to_workspace,
            crate_to_paths: crate_to_paths.clone(),
            workspaces: workspaces.clone(),
            crate_graph,
            crate_node_indices,
        })
    }

    /// Analyze which crates and workspaces are affected by the given files
    pub fn analyze_affected_files(&self, files: &[String]) -> AffectedResult {
        let mut directly_affected_crates = HashSet::new();
        let mut directly_affected_crate_paths = HashSet::new();
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
            // Try to canonicalize to resolve symlinks (e.g., /private/var -> /var on macOS)
            let abs_file = abs_file.canonicalize().unwrap_or(abs_file);

            // Try to find by checking if file is under any crate directory
            // When multiple crates match, prefer the one with the longest matching path
            let mut best_match: Option<(String, PathBuf, usize)> = None;

            for (crate_name, crate_paths) in &self.crate_to_paths {
                for crate_path in crate_paths {
                    // Normalize the crate path to absolute and resolve symlinks
                    let abs_crate = if crate_path.is_absolute() {
                        crate_path.clone()
                    } else {
                        cwd.join(crate_path)
                    };
                    // Try to canonicalize to resolve symlinks
                    let abs_crate = abs_crate.canonicalize().unwrap_or(abs_crate);

                    // Check if the file is under this crate's directory
                    if abs_file.starts_with(&abs_crate) {
                        let match_len = abs_crate.as_os_str().len();
                        match &best_match {
                            None => {
                                best_match =
                                    Some((crate_name.clone(), crate_path.clone(), match_len))
                            }
                            Some((_, _, best_len)) => {
                                if match_len > *best_len {
                                    best_match =
                                        Some((crate_name.clone(), crate_path.clone(), match_len));
                                }
                            }
                        }
                    }
                }
            }

            if let Some((crate_name, crate_path, _)) = best_match {
                directly_affected_crates.insert(crate_name.clone());
                directly_affected_crate_paths.insert((crate_name, crate_path));
            } else {
                unmatched_files.push(file.clone());
            }
        }

        // Find all crates affected by reverse dependencies
        let mut all_affected_crates = directly_affected_crates.clone();
        for crate_name in &directly_affected_crates {
            if let Some(&node_idx) = self.crate_node_indices.get(crate_name) {
                self.find_reverse_dependencies(node_idx, &mut all_affected_crates);
            }
        }

        // Map directly affected crates to workspaces using the exact paths that were
        // matched
        let directly_affected_workspaces: HashSet<String> = directly_affected_crate_paths
            .iter()
            .filter_map(|(crate_name, crate_path)| {
                self.crate_path_to_workspace
                    .get(&(crate_name.clone(), crate_path.clone()))
                    .and_then(|ws_path| self.workspaces.get(ws_path))
                    .map(|ws_info| ws_info.name().to_string())
            })
            .collect();

        // For all affected crates (including reverse dependencies), we need to find
        // their workspaces
        let all_affected_workspaces: HashSet<String> = all_affected_crates
            .iter()
            .flat_map(|crate_name| {
                // A crate might exist in multiple workspaces, so collect all of them
                self.crate_to_paths
                    .get(crate_name)
                    .map(|paths| {
                        paths
                            .iter()
                            .filter_map(|path| {
                                self.crate_path_to_workspace
                                    .get(&(crate_name.clone(), path.clone()))
                                    .and_then(|ws_path| self.workspaces.get(ws_path))
                                    .map(|ws_info| ws_info.name().to_string())
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();

        AffectedResult {
            directly_affected_crates,
            all_affected_crates,
            directly_affected_workspaces,
            all_affected_workspaces,
            unmatched_files,
        }
    }

    fn find_reverse_dependencies(&self, node_idx: NodeIndex, affected: &mut HashSet<String>) {
        use petgraph::Direction;

        for edge in self
            .crate_graph
            .edges_directed(node_idx, Direction::Incoming)
        {
            let source_idx = edge.source();
            let source_crate = &self.crate_graph[source_idx];
            if affected.insert(source_crate.clone()) {
                // Recursively find more reverse dependencies
                self.find_reverse_dependencies(source_idx, affected);
            }
        }
    }
}

pub struct AffectedResult {
    pub directly_affected_crates: HashSet<String>,
    pub all_affected_crates: HashSet<String>,
    pub directly_affected_workspaces: HashSet<String>,
    pub all_affected_workspaces: HashSet<String>,
    pub unmatched_files: Vec<String>,
}

impl AffectedResult {
    pub fn to_json_report(&self, analysis: &AffectedAnalysis) -> AffectedJsonReport {
        let mut affected_crates = Vec::new();

        for crate_name in &self.all_affected_crates {
            // Get workspace info for this crate (prefer first path found)
            let workspace_info = analysis
                .crate_to_paths
                .get(crate_name)
                .and_then(|paths| paths.first())
                .and_then(|path| {
                    analysis
                        .crate_path_to_workspace
                        .get(&(crate_name.clone(), path.clone()))
                        .and_then(|ws_path| analysis.workspaces.get(ws_path))
                });

            let (workspace_name, is_standalone) = workspace_info
                .map(|ws| (ws.name().to_string(), ws.is_standalone()))
                .unwrap_or_else(|| ("unknown".to_string(), false));

            affected_crates.push(AffectedCrate {
                name: crate_name.clone(),
                workspace: workspace_name,
                is_directly_affected: self.directly_affected_crates.contains(crate_name),
                is_standalone,
            });
        }

        // Sort affected crates by name for deterministic output
        affected_crates.sort_by(|a, b| a.name.cmp(&b.name));

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

        let mut directly_affected_crates: Vec<String> =
            self.directly_affected_crates.iter().cloned().collect();
        directly_affected_crates.sort();

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
            analyzer.crate_to_workspace(),
            analyzer.crate_to_paths(),
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

        assert!(result_a.directly_affected_crates.contains("phoenix-v2-api"));
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

        assert!(result_b.directly_affected_crates.contains("phoenix-v2-api"));
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
        assert!(result.directly_affected_crates.contains("crate-b"));
        assert_eq!(result.directly_affected_crates.len(), 1);

        // crate-a should be affected via reverse dependency
        assert!(result.all_affected_crates.contains("crate-a"));
        assert!(result.all_affected_crates.contains("crate-b"));
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

        assert!(result.directly_affected_crates.contains("crate-a"));

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
        assert!(result.directly_affected_crates.contains("crate-a"));
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
        assert!(result.directly_affected_crates.contains("consumer-crate"));

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
        assert!(result.directly_affected_crates.contains("crate-a"));
        assert!(result.directly_affected_crates.contains("crate-b"));

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

        // Workspace Cargo.toml should not map to any specific crate
        assert_eq!(result.unmatched_files.len(), 1);
        assert_eq!(
            result.unmatched_files[0],
            format!("{}/my-workspace/Cargo.toml", temp.path().display())
        );
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
        assert!(
            result
                .directly_affected_crates
                .contains("standalone-test-crate")
        );

        // Workspace Cargo.lock should not map to any specific crate
        assert!(result.unmatched_files.contains(&format!(
            "{}/real-workspace/Cargo.lock",
            temp.path().display()
        )));
    }
}
