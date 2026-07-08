//! Lineup command implementation

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use miette::{Result, WrapErr};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{EdgeRef, IntoNodeReferences};
use serde::{Deserialize, Serialize};

use crate::analyzer::{CrateWorkspaceMap, WorkspaceInfo};
use crate::cli::Commands;
use crate::common::{ConfigBuilder, FromCommand};
use crate::config::WorkspaceDepsConfig;
use crate::error::FerrisWheelError;
use crate::graph::{DependencyEdge, WorkspaceNode};

/// JSON output structure for workspace dependencies
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceDepsJsonReport {
    pub workspaces: Vec<WorkspaceDepsEntry>,
}

/// Individual workspace entry in the JSON report
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceDepsEntry {
    pub name: String,
    pub path: String,
    pub dependencies: Vec<String>,
    pub reverse: bool,
    pub transitive: bool,
    pub is_standalone: bool,
}

impl FromCommand for WorkspaceDepsConfig {
    fn from_command(command: Commands) -> Result<Self, FerrisWheelError> {
        match command {
            Commands::Lineup {
                workspace,
                reverse,
                transitive,
                common,
                format,
            } => WorkspaceDepsConfig::builder()
                .with_workspace(workspace)
                .with_reverse(reverse)
                .with_transitive(transitive)
                .with_paths(common.get_paths())
                .with_format(format.format)
                .with_exclude_dev(common.exclude_dev)
                .with_exclude_build(common.exclude_build)
                .with_exclude_target(common.exclude_target)
                .build(),
            _ => Err(FerrisWheelError::ConfigurationError {
                message: "Invalid command type for WorkspaceDepsConfig".to_string(),
            }),
        }
    }
}

crate::impl_try_from_command!(WorkspaceDepsConfig);

/// Execute the lineup command for analyzing workspace dependencies
pub fn execute_deps_command(command: Commands) -> Result<()> {
    let config = WorkspaceDepsConfig::from_command(command)
        .wrap_err("Failed to parse lineup command configuration")?;

    use crate::executors::CommandExecutor;
    use crate::executors::deps::DepsExecutor;
    DepsExecutor::execute(config)
}

/// Analysis of workspace dependencies
pub struct WorkspaceDependencyAnalysis {
    workspaces: HashMap<PathBuf, WorkspaceInfo>,
    graph: DiGraph<WorkspaceNode, DependencyEdge>,
    node_indices: HashMap<String, NodeIndex>,
    node_indices_by_path: HashMap<PathBuf, NodeIndex>,
    // Cache for computed dependencies
    direct_deps_cache: HashMap<String, HashSet<String>>,
    reverse_deps_cache: HashMap<String, HashSet<String>>,
    transitive_deps_cache: HashMap<String, HashSet<String>>,
}

#[derive(Debug, Clone)]
struct WorkspaceReportEntry {
    name: String,
    path: Option<PathBuf>,
    is_standalone: bool,
}

impl WorkspaceDependencyAnalysis {
    pub fn new(
        workspaces: &HashMap<PathBuf, WorkspaceInfo>,
        _crate_to_workspace: &CrateWorkspaceMap,
        graph: &DiGraph<WorkspaceNode, DependencyEdge>,
    ) -> Self {
        // Build node index lookup
        let mut node_indices = HashMap::new();
        let mut node_indices_by_path = HashMap::new();
        for (idx, node) in graph.node_references() {
            node_indices.insert(node.name(), idx);
            if let Some(path) = node.path() {
                node_indices_by_path.insert(path.to_path_buf(), idx);
            }
        }

        Self {
            workspaces: workspaces.clone(),
            graph: graph.clone(),
            node_indices: node_indices
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            node_indices_by_path,
            direct_deps_cache: HashMap::new(),
            reverse_deps_cache: HashMap::new(),
            transitive_deps_cache: HashMap::new(),
        }
    }

    /// Get all workspace names
    pub fn workspace_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .workspaces
            .values()
            .map(|ws| ws.name().to_string())
            .collect();
        names.sort();
        names
    }

    /// Get workspace info by name
    pub fn get_workspace_info(&self, workspace_name: &str) -> Option<&WorkspaceInfo> {
        self.workspaces
            .values()
            .find(|ws| ws.name() == workspace_name)
    }

    /// Get workspace path by name
    pub fn get_workspace_path(&self, workspace_name: &str) -> Option<&PathBuf> {
        self.workspaces
            .iter()
            .find(|(_, ws)| ws.name() == workspace_name)
            .map(|(path, _)| path)
    }

    /// Get direct dependencies of a workspace
    pub fn get_direct_dependencies(&mut self, workspace: &str) -> &HashSet<String> {
        if !self.direct_deps_cache.contains_key(workspace) {
            let deps = self.direct_dependencies_for(None, workspace);

            self.direct_deps_cache.insert(workspace.to_string(), deps);
        }

        &self.direct_deps_cache[workspace]
    }

    /// Get workspaces that depend on this workspace (reverse dependencies)
    pub fn get_reverse_dependencies(&mut self, workspace: &str) -> &HashSet<String> {
        if !self.reverse_deps_cache.contains_key(workspace) {
            let deps = self.reverse_dependencies_for(None, workspace);

            self.reverse_deps_cache.insert(workspace.to_string(), deps);
        }

        &self.reverse_deps_cache[workspace]
    }

    /// Get all transitive dependencies of a workspace using DFS
    pub fn get_transitive_dependencies(&mut self, workspace: &str) -> &HashSet<String> {
        if !self.transitive_deps_cache.contains_key(workspace) {
            let deps = self.transitive_dependencies_for(None, workspace);

            self.transitive_deps_cache
                .insert(workspace.to_string(), deps);
        }

        &self.transitive_deps_cache[workspace]
    }

    fn node_index_for(
        &self,
        workspace_path: Option<&Path>,
        workspace_name: &str,
    ) -> Option<NodeIndex> {
        workspace_path
            .and_then(|path| self.node_indices_by_path.get(path).copied())
            .or_else(|| self.node_indices.get(workspace_name).copied())
    }

    fn direct_dependencies_for(
        &self,
        workspace_path: Option<&Path>,
        workspace_name: &str,
    ) -> HashSet<String> {
        let mut deps = HashSet::new();

        if let Some(node_idx) = self.node_index_for(workspace_path, workspace_name) {
            for edge in self.graph.edges(node_idx) {
                let target_node = &self.graph[edge.target()];
                deps.insert(target_node.name().to_string());
            }
        }

        deps
    }

    fn reverse_dependencies_for(
        &self,
        workspace_path: Option<&Path>,
        workspace_name: &str,
    ) -> HashSet<String> {
        let mut deps = HashSet::new();

        if let Some(node_idx) = self.node_index_for(workspace_path, workspace_name) {
            for edge in self.graph.edges_directed(node_idx, petgraph::Incoming) {
                let source_node = &self.graph[edge.source()];
                deps.insert(source_node.name().to_string());
            }
        }

        deps
    }

    fn transitive_dependencies_for(
        &self,
        workspace_path: Option<&Path>,
        workspace_name: &str,
    ) -> HashSet<String> {
        let mut visited = HashSet::new();
        let mut deps = HashSet::new();

        if let Some(node_idx) = self.node_index_for(workspace_path, workspace_name) {
            self.dfs_dependencies(node_idx, &mut visited, &mut deps);
        }

        deps
    }

    /// Depth-first search to find all transitive dependencies
    fn dfs_dependencies(
        &self,
        node_idx: NodeIndex,
        visited: &mut HashSet<NodeIndex>,
        deps: &mut HashSet<String>,
    ) {
        if visited.contains(&node_idx) {
            return;
        }
        visited.insert(node_idx);

        for edge in self.graph.edges(node_idx) {
            let target = edge.target();
            let target_node = &self.graph[target];
            deps.insert(target_node.name().to_string());
            self.dfs_dependencies(target, visited, deps);
        }
    }
}

/// Report generator for workspace dependency analysis
pub struct WorkspaceDepsReportGenerator {
    workspace_filter: Option<String>,
    reverse: bool,
    transitive: bool,
}

impl WorkspaceDepsReportGenerator {
    pub fn new(workspace: Option<&str>, reverse: bool, transitive: bool) -> Self {
        Self {
            workspace_filter: workspace.map(|s| s.to_string()),
            reverse,
            transitive,
        }
    }

    pub fn generate_human_report(
        &self,
        analysis: &mut WorkspaceDependencyAnalysis,
    ) -> Result<String, FerrisWheelError> {
        let mut output = String::new();

        let workspaces = self.selected_workspace_entries(analysis);

        for workspace in workspaces {
            writeln!(output, "\n📦 Workspace: {}", workspace.name)?;

            // Add workspace path if available
            if let Some(workspace_path) = &workspace.path {
                writeln!(output, "  📍 Path: {}", workspace_path.display())?;
            }

            let deps = self.dependencies_for_entry(analysis, &workspace);

            if self.reverse {
                writeln!(output, "  ⬆️  Reverse dependencies (who depends on this):")?;
                if deps.is_empty() {
                    writeln!(output, "    (none)")?;
                } else {
                    let mut sorted_deps: Vec<_> = deps.into_iter().collect();
                    sorted_deps.sort();
                    for dep in sorted_deps {
                        writeln!(output, "    - {dep}")?;
                    }
                }
            } else if self.transitive {
                writeln!(output, "  ⬇️  All transitive dependencies:")?;
                if deps.is_empty() {
                    writeln!(output, "    (none)")?;
                } else {
                    let mut sorted_deps: Vec<_> = deps.into_iter().collect();
                    sorted_deps.sort();
                    for dep in sorted_deps {
                        writeln!(output, "    - {dep}")?;
                    }
                }
            } else {
                writeln!(output, "  ⬇️  Direct dependencies:")?;
                if deps.is_empty() {
                    writeln!(output, "    (none)")?;
                } else {
                    let mut sorted_deps: Vec<_> = deps.into_iter().collect();
                    sorted_deps.sort();
                    for dep in sorted_deps {
                        writeln!(output, "    - {dep}")?;
                    }
                }
            }
        }

        Ok(output)
    }

    pub fn generate_json_report(
        &self,
        analysis: &mut WorkspaceDependencyAnalysis,
    ) -> Result<String, FerrisWheelError> {
        let workspaces = self.selected_workspace_entries(analysis);

        let mut workspace_data = Vec::new();

        for workspace in workspaces {
            let deps = self.dependencies_for_entry(analysis, &workspace);

            let workspace_path = workspace
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(unknown)".to_string());

            let mut sorted_deps = deps.into_iter().collect::<Vec<_>>();
            sorted_deps.sort();

            workspace_data.push(WorkspaceDepsEntry {
                name: workspace.name,
                path: workspace_path,
                dependencies: sorted_deps,
                reverse: self.reverse,
                transitive: self.transitive,
                is_standalone: workspace.is_standalone,
            });
        }

        workspace_data.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));

        let report = WorkspaceDepsJsonReport {
            workspaces: workspace_data,
        };

        Ok(serde_json::to_string_pretty(&report)?)
    }

    pub fn generate_junit_report(
        &self,
        analysis: &mut WorkspaceDependencyAnalysis,
    ) -> Result<String, FerrisWheelError> {
        let mut output = String::new();

        writeln!(output, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        writeln!(
            output,
            r#"<testsuites name="workspace-dependencies" tests="1" failures="0">"#
        )?;
        writeln!(
            output,
            r#"  <testsuite name="dependency-analysis" tests="1" failures="0">"#
        )?;
        writeln!(
            output,
            r#"    <testcase name="analyze-workspace-dependencies" classname="ferris-wheel">"#
        )?;

        let workspaces = self.selected_workspace_entries(analysis);

        writeln!(output, "Workspace dependency analysis results:")?;
        for workspace in workspaces {
            let deps = self.dependencies_for_entry(analysis, &workspace);

            writeln!(output, "  {}: {} dependencies", workspace.name, deps.len())?;
        }

        writeln!(output, r#"    </testcase>"#)?;
        writeln!(output, r#"  </testsuite>"#)?;
        writeln!(output, r#"</testsuites>"#)?;

        Ok(output)
    }

    pub fn generate_github_report(
        &self,
        analysis: &mut WorkspaceDependencyAnalysis,
    ) -> Result<String, FerrisWheelError> {
        let mut output = String::new();

        let workspaces = self.selected_workspace_entries(analysis);

        writeln!(
            output,
            "::notice title=Workspace Dependencies::Analyzed {} workspace{}",
            workspaces.len(),
            if workspaces.len() == 1 { "" } else { "s" }
        )?;

        for workspace in workspaces {
            let deps = self.dependencies_for_entry(analysis, &workspace);

            let dep_type = if self.reverse {
                "reverse"
            } else if self.transitive {
                "transitive"
            } else {
                "direct"
            };

            let dep_count = deps.len();
            let mut sorted_deps: Vec<_> = deps.into_iter().collect();
            sorted_deps.sort();

            writeln!(
                output,
                "::notice title={}::{} {} dependencies: {}",
                workspace.name,
                dep_count,
                dep_type,
                sorted_deps.join(", ")
            )?;
        }

        Ok(output)
    }

    fn selected_workspace_entries(
        &self,
        analysis: &WorkspaceDependencyAnalysis,
    ) -> Vec<WorkspaceReportEntry> {
        let mut entries: Vec<_> = analysis
            .workspaces
            .iter()
            .filter(|(_, workspace)| {
                self.workspace_filter
                    .as_ref()
                    .is_none_or(|filter| workspace.name() == filter)
            })
            .map(|(path, workspace)| WorkspaceReportEntry {
                name: workspace.name().to_string(),
                path: Some(path.clone()),
                is_standalone: workspace.is_standalone(),
            })
            .collect();

        entries.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));

        if entries.is_empty()
            && let Some(filter) = &self.workspace_filter
        {
            entries.push(WorkspaceReportEntry {
                name: filter.clone(),
                path: None,
                is_standalone: false,
            });
        }

        entries
    }

    fn dependencies_for_entry(
        &self,
        analysis: &WorkspaceDependencyAnalysis,
        workspace: &WorkspaceReportEntry,
    ) -> HashSet<String> {
        let path = workspace.path.as_deref();

        if self.reverse {
            analysis.reverse_dependencies_for(path, &workspace.name)
        } else if self.transitive {
            analysis.transitive_dependencies_for(path, &workspace.name)
        } else {
            analysis.direct_dependencies_for(path, &workspace.name)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    use petgraph::graph::DiGraph;

    use super::*;
    use crate::analyzer::{CrateWorkspaceMap, WorkspaceInfo};
    use crate::graph::{DependencyEdge, WorkspaceNode};

    fn create_test_graph() -> (
        DiGraph<WorkspaceNode, DependencyEdge>,
        HashMap<PathBuf, WorkspaceInfo>,
        CrateWorkspaceMap,
    ) {
        let mut graph = DiGraph::new();
        let mut workspaces = HashMap::new();
        let crate_to_workspace = CrateWorkspaceMap::new();

        // Create workspace nodes
        let node_a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a".to_string()])
                .build()
                .unwrap(),
        );

        let node_b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b".to_string()])
                .build()
                .unwrap(),
        );

        let node_c = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-c".to_string())
                .with_crates(vec!["crate-c".to_string()])
                .build()
                .unwrap(),
        );

        // Add edges: A -> B, B -> C
        graph.add_edge(
            node_a,
            node_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a")
                .with_to_crate("crate-b")
                .with_dependency_type(crate::graph::DependencyType::Normal)
                .build()
                .unwrap(),
        );

        graph.add_edge(
            node_b,
            node_c,
            DependencyEdge::builder()
                .with_from_crate("crate-b")
                .with_to_crate("crate-c")
                .with_dependency_type(crate::graph::DependencyType::Normal)
                .build()
                .unwrap(),
        );

        // Create mock workspace info
        let path_a = PathBuf::from("/test/workspace-a");
        let path_b = PathBuf::from("/test/workspace-b");
        let path_c = PathBuf::from("/test/workspace-c");

        workspaces.insert(
            path_a.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-a")
                .with_members(vec![])
                .build()
                .unwrap(),
        );

        workspaces.insert(
            path_b.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-b")
                .with_members(vec![])
                .build()
                .unwrap(),
        );

        workspaces.insert(
            path_c.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-c")
                .with_members(vec![])
                .build()
                .unwrap(),
        );

        (graph, workspaces, crate_to_workspace)
    }

    #[test]
    fn test_direct_dependencies() {
        let (graph, workspaces, crate_to_workspace) = create_test_graph();
        let mut analysis =
            WorkspaceDependencyAnalysis::new(&workspaces, &crate_to_workspace, &graph);

        // Test direct dependencies
        let deps_a = analysis.get_direct_dependencies("workspace-a");
        assert_eq!(deps_a.len(), 1);
        assert!(deps_a.contains("workspace-b"));

        let deps_b = analysis.get_direct_dependencies("workspace-b");
        assert_eq!(deps_b.len(), 1);
        assert!(deps_b.contains("workspace-c"));

        let deps_c = analysis.get_direct_dependencies("workspace-c");
        assert_eq!(deps_c.len(), 0);
    }

    #[test]
    fn test_reverse_dependencies() {
        let (graph, workspaces, crate_to_workspace) = create_test_graph();
        let mut analysis =
            WorkspaceDependencyAnalysis::new(&workspaces, &crate_to_workspace, &graph);

        // Test reverse dependencies
        let rev_deps_a = analysis.get_reverse_dependencies("workspace-a");
        assert_eq!(rev_deps_a.len(), 0);

        let rev_deps_b = analysis.get_reverse_dependencies("workspace-b");
        assert_eq!(rev_deps_b.len(), 1);
        assert!(rev_deps_b.contains("workspace-a"));

        let rev_deps_c = analysis.get_reverse_dependencies("workspace-c");
        assert_eq!(rev_deps_c.len(), 1);
        assert!(rev_deps_c.contains("workspace-b"));
    }

    #[test]
    fn test_transitive_dependencies() {
        let (graph, workspaces, crate_to_workspace) = create_test_graph();
        let mut analysis =
            WorkspaceDependencyAnalysis::new(&workspaces, &crate_to_workspace, &graph);

        // Test transitive dependencies
        let trans_deps_a = analysis.get_transitive_dependencies("workspace-a");
        assert_eq!(trans_deps_a.len(), 2);
        assert!(trans_deps_a.contains("workspace-b"));
        assert!(trans_deps_a.contains("workspace-c"));

        let trans_deps_b = analysis.get_transitive_dependencies("workspace-b");
        assert_eq!(trans_deps_b.len(), 1);
        assert!(trans_deps_b.contains("workspace-c"));

        let trans_deps_c = analysis.get_transitive_dependencies("workspace-c");
        assert_eq!(trans_deps_c.len(), 0);
    }

    #[test]
    fn test_human_report_generator() {
        let (graph, workspaces, crate_to_workspace) = create_test_graph();
        let mut analysis =
            WorkspaceDependencyAnalysis::new(&workspaces, &crate_to_workspace, &graph);

        let generator = WorkspaceDepsReportGenerator::new(Some("workspace-a"), false, false);
        let report = generator.generate_human_report(&mut analysis).unwrap();

        assert!(report.contains("workspace-a"));
        assert!(report.contains("Path: /test/workspace-a"));
        assert!(report.contains("Direct dependencies"));
        assert!(report.contains("workspace-b"));
    }

    #[test]
    fn test_json_report_generator() {
        let (graph, workspaces, crate_to_workspace) = create_test_graph();
        let mut analysis =
            WorkspaceDependencyAnalysis::new(&workspaces, &crate_to_workspace, &graph);

        let generator = WorkspaceDepsReportGenerator::new(None, false, false);
        let report = generator.generate_json_report(&mut analysis).unwrap();

        let json: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert!(json["workspaces"].is_array());

        // Verify path field exists in the JSON output
        let workspace_deps = json["workspaces"].as_array().unwrap();
        assert!(!workspace_deps.is_empty());
        assert!(workspace_deps[0]["path"].is_string());
    }

    #[test]
    fn test_json_report_preserves_paths_for_duplicate_workspace_names() {
        let mut graph = DiGraph::new();
        let main_tools_path = PathBuf::from("/test/main/tools");
        let standalone_tools_path = PathBuf::from("/test/standalone-runner");

        graph.add_node(
            WorkspaceNode::builder()
                .with_name("tools".to_string())
                .with_path(main_tools_path.clone())
                .with_crates(vec!["tools-cli".to_string()])
                .build()
                .unwrap(),
        );
        graph.add_node(
            WorkspaceNode::builder()
                .with_name("tools".to_string())
                .with_path(standalone_tools_path.clone())
                .with_crates(vec!["tools".to_string()])
                .build()
                .unwrap(),
        );

        let mut workspaces = HashMap::new();
        workspaces.insert(
            main_tools_path.clone(),
            WorkspaceInfo::builder()
                .with_name("tools")
                .with_members(vec![])
                .build()
                .unwrap(),
        );
        workspaces.insert(
            standalone_tools_path.clone(),
            WorkspaceInfo::builder()
                .with_name("tools")
                .with_members(vec![])
                .with_is_standalone(true)
                .build()
                .unwrap(),
        );

        let mut analysis =
            WorkspaceDependencyAnalysis::new(&workspaces, &CrateWorkspaceMap::new(), &graph);
        let report = WorkspaceDepsReportGenerator::new(None, false, false)
            .generate_json_report(&mut analysis)
            .unwrap();
        let json: WorkspaceDepsJsonReport = serde_json::from_str(&report).unwrap();

        let paths: HashSet<_> = json
            .workspaces
            .iter()
            .map(|workspace| workspace.path.as_str())
            .collect();

        assert_eq!(json.workspaces.len(), 2);
        assert!(paths.contains("/test/main/tools"));
        assert!(paths.contains("/test/standalone-runner"));
    }
}
