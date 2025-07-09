use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::{Result, WrapErr};
use petgraph::graph::{DiGraph, NodeIndex};

use super::types::{DependencyEdge, DependencyType, WorkspaceNode};
use crate::analyzer::{Dependency, DependencyBuilder, WorkspaceInfo};
use crate::common::ConfigBuilder;
use crate::dependency_filter::DependencyFilter;
use crate::progress::ProgressReporter;
use crate::toml_parser::CargoToml;

/// Builder for constructing dependency graphs
///
/// This builder creates directed graphs representing dependencies between
/// workspaces or crates, with support for filtering different dependency types.
pub struct DependencyGraphBuilder {
    graph: DiGraph<WorkspaceNode, DependencyEdge>,
    workspace_indices: HashMap<PathBuf, NodeIndex>,
    filter: DependencyFilter,
    // Cache for resolved dependencies
    workspace_dependencies_cache: HashMap<PathBuf, HashMap<String, PathBuf>>,
}

// Types are now imported from the types module

impl DependencyGraphBuilder {
    /// Create a new dependency graph builder
    ///
    /// # Arguments
    /// * `exclude_dev` - Exclude dev dependencies from the graph
    /// * `exclude_build` - Exclude build dependencies from the graph
    /// * `exclude_target` - Exclude target-specific dependencies from the graph
    pub fn new(exclude_dev: bool, exclude_build: bool, exclude_target: bool) -> Self {
        Self {
            graph: DiGraph::new(),
            workspace_indices: HashMap::new(),
            filter: DependencyFilter::new(exclude_dev, exclude_build, exclude_target),
            workspace_dependencies_cache: HashMap::new(),
        }
    }

    /// Check if a dependency type should be included based on the filter
    /// settings
    fn should_include_dependency_type(&self, dep_type: &DependencyType) -> bool {
        match dep_type {
            DependencyType::Normal => true, // Normal deps are always included
            DependencyType::Dev => self.filter.include_dev(),
            DependencyType::Build => self.filter.include_build(),
        }
    }

    /// Build a graph showing dependencies between crates within workspaces
    ///
    /// This creates a fine-grained graph where each crate is a node,
    /// useful for detecting cycles within individual workspaces.
    pub fn build_intra_workspace_graph(
        &mut self,
        workspaces: &HashMap<PathBuf, WorkspaceInfo>,
        progress: Option<&ProgressReporter>,
    ) -> Result<()> {
        // Create a crate-level graph for detecting cycles within workspaces
        // Each crate becomes a node, edges represent dependencies between crates in the
        // same workspace

        let mut crate_indices: HashMap<String, NodeIndex> = HashMap::new();

        // First, create nodes for all crates, grouped by workspace
        for ws_info in workspaces.values() {
            if let Some(p) = progress {
                p.analyzing_workspace(ws_info.name());
            }

            for member in ws_info.members() {
                let node = WorkspaceNode::builder()
                    .with_name(format!("{}/{}", ws_info.name(), member.name()))
                    .with_crates(vec![member.name().to_string()])
                    .build()
                    .wrap_err("Failed to build WorkspaceNode")?;

                let idx = self.graph.add_node(node);
                crate_indices.insert(member.name().to_string(), idx);
            }
        }

        // Then, analyze dependencies within each workspace
        for (ws_path, ws_info) in workspaces {
            for member in ws_info.members() {
                let from_idx = crate_indices[member.name()];

                // Process all dependency types to find intra-workspace cycles
                let all_deps = [
                    (member.dependencies(), DependencyType::Normal),
                    (member.dev_dependencies(), DependencyType::Dev),
                    (member.build_dependencies(), DependencyType::Build),
                ];

                for (deps, dep_type) in all_deps {
                    // Skip excluded dependency types
                    if !self.should_include_dependency_type(&dep_type) {
                        continue;
                    }

                    for dep in deps {
                        // Skip if this specific dependency should be filtered out (e.g.,
                        // target-specific)
                        if !self.filter.should_include_dependency(dep) {
                            continue;
                        }

                        // Only process if this dependency points to another crate in the same
                        // workspace
                        if let Some(dep_crate_idx) = crate_indices.get(dep.name()) {
                            // Check if it's in the same workspace
                            let dep_workspace = workspaces
                                .iter()
                                .find(|(_, ws)| ws.members().iter().any(|m| m.name() == dep.name()))
                                .map(|(path, _)| path);

                            if dep_workspace == Some(ws_path) {
                                let edge = DependencyEdge::builder()
                                    .with_from_crate(member.name())
                                    .with_to_crate(dep.name())
                                    .with_dependency_type(dep_type.clone())
                                    .with_target(dep.target().map(|t| t.to_string()))
                                    .build()
                                    .wrap_err("Failed to build DependencyEdge")?;

                                self.graph.add_edge(from_idx, *dep_crate_idx, edge);
                            }
                        }
                    }
                }

                // Process target-specific dependencies
                for (target, deps) in member.target_dependencies() {
                    for dep in deps {
                        // Skip if target dependencies are excluded or this specific dependency
                        // should be filtered
                        if !self.filter.include_target()
                            || !self.filter.should_include_dependency(dep)
                        {
                            continue;
                        }

                        if let Some(dep_crate_idx) = crate_indices.get(dep.name()) {
                            // Check if it's in the same workspace
                            let dep_workspace = workspaces
                                .iter()
                                .find(|(_, ws)| ws.members().iter().any(|m| m.name() == dep.name()))
                                .map(|(path, _)| path);

                            if dep_workspace == Some(ws_path) {
                                let edge = DependencyEdge::builder()
                                    .with_from_crate(member.name())
                                    .with_to_crate(dep.name())
                                    .with_dependency_type(DependencyType::Normal) // Target deps are treated as normal
                                    .with_target(Some(target.clone()))
                                    .build()
                                    .wrap_err("Failed to build DependencyEdge")?;

                                self.graph.add_edge(from_idx, *dep_crate_idx, edge);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn build_cross_workspace_graph(
        &mut self,
        workspaces: &HashMap<PathBuf, WorkspaceInfo>,
        crate_to_workspace: &HashMap<String, PathBuf>,
        progress: Option<&ProgressReporter>,
    ) -> Result<()> {
        // First, create nodes for all workspaces
        for (ws_path, ws_info) in workspaces {
            let node = WorkspaceNode::builder()
                .with_name(ws_info.name().to_string())
                .with_crates(
                    ws_info
                        .members()
                        .iter()
                        .map(|m| m.name().to_string())
                        .collect(),
                )
                .build()
                .wrap_err("Failed to build WorkspaceNode")?;

            let idx = self.graph.add_node(node);
            self.workspace_indices.insert(ws_path.clone(), idx);
        }

        // Load workspace dependencies for each workspace
        self.load_workspace_dependencies(workspaces)?;

        // Then, analyze dependencies and create edges
        for (ws_path, ws_info) in workspaces {
            if let Some(p) = progress {
                p.analyzing_workspace(ws_info.name());
            }

            let from_idx = self.workspace_indices[ws_path];

            // Check each crate in this workspace
            for member in ws_info.members() {
                // Process normal dependencies (always included)
                for dep in member.dependencies() {
                    self.process_dependency(
                        from_idx,
                        member.name(),
                        dep,
                        DependencyType::Normal,
                        crate_to_workspace,
                        ws_path,
                    )
                    .wrap_err_with(|| {
                        format!(
                            "Failed to process dependency '{}' for crate '{}'",
                            dep.name(),
                            member.name()
                        )
                    })?;
                }

                // Process dev dependencies unless excluded
                if self.filter.include_dev() {
                    for dep in member.dev_dependencies() {
                        self.process_dependency(
                            from_idx,
                            member.name(),
                            dep,
                            DependencyType::Dev,
                            crate_to_workspace,
                            ws_path,
                        )
                        .wrap_err_with(|| {
                            format!(
                                "Failed to process dev dependency '{}' for crate '{}'",
                                dep.name(),
                                member.name()
                            )
                        })?;
                    }
                }

                // Process build dependencies unless excluded
                if self.filter.include_build() {
                    for dep in member.build_dependencies() {
                        self.process_dependency(
                            from_idx,
                            member.name(),
                            dep,
                            DependencyType::Build,
                            crate_to_workspace,
                            ws_path,
                        )
                        .wrap_err_with(|| {
                            format!(
                                "Failed to process build dependency '{}' for crate '{}'",
                                dep.name(),
                                member.name()
                            )
                        })?;
                    }
                }

                // Process target-specific dependencies unless excluded
                if self.filter.include_target() {
                    for (target, deps) in member.target_dependencies() {
                        for dep in deps {
                            let dep = DependencyBuilder::from(dep)
                                .with_target(target.clone())
                                .build()?;
                            self.process_dependency(
                                from_idx,
                                member.name(),
                                &dep,
                                DependencyType::Normal,
                                crate_to_workspace,
                                ws_path,
                            )
                            .wrap_err_with(|| {
                                format!(
                                    "Failed to process target dependency '{}' for crate '{}' \
                                     (target: {})",
                                    dep.name(),
                                    member.name(),
                                    target
                                )
                            })?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn load_workspace_dependencies(
        &mut self,
        workspaces: &HashMap<PathBuf, WorkspaceInfo>,
    ) -> Result<()> {
        // For each workspace, load its workspace-level dependencies
        for ws_path in workspaces.keys() {
            let workspace_toml_path = ws_path.join("Cargo.toml");
            if workspace_toml_path.exists()
                && let Ok(cargo_toml) = CargoToml::parse_file(&workspace_toml_path)
            {
                let deps = cargo_toml.get_workspace_dependencies();
                self.workspace_dependencies_cache
                    .insert(ws_path.clone(), deps);
            }
        }
        Ok(())
    }

    fn process_dependency(
        &mut self,
        from_ws_idx: NodeIndex,
        from_crate: &str,
        dep: &Dependency,
        dep_type: DependencyType,
        crate_to_workspace: &HashMap<String, PathBuf>,
        current_workspace_path: &Path,
    ) -> Result<()> {
        // Skip if this specific dependency should be filtered out (e.g.,
        // target-specific)
        if !self.filter.should_include_dependency(dep) {
            return Ok(());
        }

        // First check if this dependency directly maps to a known crate
        if let Some(target_ws_path) = crate_to_workspace.get(dep.name()) {
            // Get the target workspace index
            if let Some(&to_ws_idx) = self.workspace_indices.get(target_ws_path) {
                // Don't create self-edges (dependencies within the same workspace)
                if from_ws_idx != to_ws_idx {
                    // Check if edge already exists, if not create it
                    let edge = DependencyEdge::builder()
                        .with_from_crate(from_crate)
                        .with_to_crate(dep.name())
                        .with_dependency_type(dep_type)
                        .with_target(dep.target().map(|t| t.to_string()))
                        .build()
                        .wrap_err("Failed to build DependencyEdge")?;

                    self.graph.add_edge(from_ws_idx, to_ws_idx, edge);
                }
            }
        } else {
            // If not found directly, it might be a workspace dependency
            // Check if this workspace has a mapping for this dependency
            if let Some(workspace_deps) = self
                .workspace_dependencies_cache
                .get(current_workspace_path)
                && let Some(dep_path) = workspace_deps.get(dep.name())
            {
                // This is a workspace dependency with a path
                // Resolve the absolute path
                let abs_dep_path = if dep_path.is_relative() {
                    current_workspace_path.join(dep_path)
                } else {
                    dep_path.clone()
                };

                // Now check if this path maps to a known workspace
                // We need to check if abs_dep_path is inside any workspace
                for (crate_name, ws_path) in crate_to_workspace {
                    // Check if the dependency path contains a crate from this workspace
                    if (abs_dep_path
                        .to_string_lossy()
                        .contains(&crate_name.replace('-', "_"))
                        || abs_dep_path.to_string_lossy().contains(crate_name))
                        && let Some(&to_ws_idx) = self.workspace_indices.get(ws_path)
                        && from_ws_idx != to_ws_idx
                    {
                        let edge = DependencyEdge::builder()
                            .with_from_crate(from_crate)
                            .with_to_crate(crate_name)
                            .with_dependency_type(dep_type.clone())
                            .with_target(dep.target().map(|t| t.to_string()))
                            .build()
                            .wrap_err("Failed to build DependencyEdge")?;
                        self.graph.add_edge(from_ws_idx, to_ws_idx, edge);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn graph(&self) -> &DiGraph<WorkspaceNode, DependencyEdge> {
        &self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{CrateMember, WorkspaceInfo};

    // Helper function for creating test CrateMember using the builder
    fn test_crate_member(
        name: &str,
        workspace_path: &Path,
        dependencies: Vec<Dependency>,
    ) -> CrateMember {
        CrateMember::builder()
            .with_name(name)
            .with_path(workspace_path.join(name))
            .with_dependencies(dependencies)
            .build()
            .unwrap()
    }

    #[test]
    fn test_build_simple_graph() {
        let mut workspaces = HashMap::new();
        let mut crate_to_workspace = HashMap::new();

        // Workspace A
        let ws_a_path = PathBuf::from("/test/workspace-a");
        workspaces.insert(
            ws_a_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-a")
                .with_members(vec![test_crate_member(
                    "crate-a",
                    &ws_a_path,
                    vec![Dependency::builder().with_name("crate-b").build().unwrap()],
                )])
                .build()
                .unwrap(),
        );
        crate_to_workspace.insert("crate-a".to_string(), ws_a_path.clone());

        // Workspace B
        let ws_b_path = PathBuf::from("/test/workspace-b");
        workspaces.insert(
            ws_b_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-b")
                .with_members(vec![test_crate_member("crate-b", &ws_b_path, vec![])])
                .build()
                .unwrap(),
        );
        crate_to_workspace.insert("crate-b".to_string(), ws_b_path.clone());

        let mut builder = DependencyGraphBuilder::new(false, false, false);
        builder
            .build_cross_workspace_graph(&workspaces, &crate_to_workspace, None)
            .unwrap();

        assert_eq!(builder.graph.node_count(), 2);
        assert_eq!(builder.graph.edge_count(), 1);
    }

    #[test]
    fn test_build_intra_workspace_graph() {
        let mut workspaces = HashMap::new();

        // Single workspace with internal dependencies and a cycle
        let ws_a_path = PathBuf::from("/test/workspace-a");
        workspaces.insert(
            ws_a_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-a")
                .with_members(vec![
                    CrateMember::builder()
                        .with_name("crate-a")
                        .with_path(ws_a_path.join("crate-a"))
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-b").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-b")
                        .with_path(ws_a_path.join("crate-b"))
                        .with_dev_dependencies(vec![
                            Dependency::builder().with_name("crate-a").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                ])
                .build()
                .unwrap(),
        );

        let mut builder = DependencyGraphBuilder::new(false, false, false);
        builder
            .build_intra_workspace_graph(&workspaces, None)
            .unwrap();

        // Should create 2 nodes (one for each crate) and 2 edges (forming a cycle)
        assert_eq!(builder.graph.node_count(), 2);
        assert_eq!(builder.graph.edge_count(), 2);

        // Verify the nodes are named correctly
        let node_names: Vec<String> = builder
            .graph
            .node_weights()
            .map(|node| node.name().to_string())
            .collect();
        assert!(node_names.contains(&"workspace-a/crate-a".to_string()));
        assert!(node_names.contains(&"workspace-a/crate-b".to_string()));
    }

    #[test]
    fn test_intra_workspace_no_cycles_between_workspaces() {
        let mut workspaces = HashMap::new();

        // Two workspaces, each with internal dependencies but no cross-workspace deps
        let ws_a_path = PathBuf::from("/test/workspace-a");
        workspaces.insert(
            ws_a_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-a")
                .with_members(vec![
                    CrateMember::builder()
                        .with_name("crate-a1")
                        .with_path(ws_a_path.join("crate-a1"))
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-a2").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-a2")
                        .with_path(ws_a_path.join("crate-a2"))
                        .build()
                        .unwrap(),
                ])
                .build()
                .unwrap(),
        );

        let ws_b_path = PathBuf::from("/test/workspace-b");
        workspaces.insert(
            ws_b_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-b")
                .with_members(vec![
                    CrateMember::builder()
                        .with_name("crate-b1")
                        .with_path(ws_b_path.join("crate-b1"))
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-b2").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-b2")
                        .with_path(ws_b_path.join("crate-b2"))
                        .build()
                        .unwrap(),
                ])
                .build()
                .unwrap(),
        );

        let mut builder = DependencyGraphBuilder::new(false, false, false);
        builder
            .build_intra_workspace_graph(&workspaces, None)
            .unwrap();

        // Should create 4 nodes but only 2 edges (no cross-workspace dependencies)
        assert_eq!(builder.graph.node_count(), 4);
        assert_eq!(builder.graph.edge_count(), 2);
    }

    #[test]
    fn test_intra_workspace_complex_cycles() {
        let mut workspaces = HashMap::new();

        // Create a workspace with complex internal cycles
        let ws_a_path = PathBuf::from("/test/workspace-a");
        workspaces.insert(
            ws_a_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-a")
                .with_members(vec![
                    CrateMember::builder()
                        .with_name("crate-a")
                        .with_path(ws_a_path.join("crate-a"))
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-b").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-b")
                        .with_path(ws_a_path.join("crate-b"))
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-c").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-c")
                        .with_path(ws_a_path.join("crate-c"))
                        .with_dev_dependencies(vec![
                            Dependency::builder().with_name("crate-a").build().unwrap(),
                        ])
                        .with_build_dependencies(vec![
                            Dependency::builder().with_name("crate-b").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                ])
                .build()
                .unwrap(),
        );

        let mut builder = DependencyGraphBuilder::new(false, false, false);
        builder
            .build_intra_workspace_graph(&workspaces, None)
            .unwrap();

        // Should create 3 nodes with multiple edges forming cycles
        assert_eq!(builder.graph.node_count(), 3);
        assert!(
            builder.graph.edge_count() >= 3,
            "Should have at least 3 edges for the cycles"
        );

        // Verify the nodes are named correctly
        let node_names: Vec<String> = builder
            .graph
            .node_weights()
            .map(|node| node.name().to_string())
            .collect();
        assert!(node_names.contains(&"workspace-a/crate-a".to_string()));
        assert!(node_names.contains(&"workspace-a/crate-b".to_string()));
        assert!(node_names.contains(&"workspace-a/crate-c".to_string()));
    }

    #[test]
    fn test_intra_workspace_mixed_dependency_types() {
        let mut workspaces = HashMap::new();

        // Test intra-workspace cycles with different dependency types
        let ws_a_path = PathBuf::from("/test/workspace-a");
        workspaces.insert(
            ws_a_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-a")
                .with_members(vec![
                    CrateMember::builder()
                        .with_name("crate-a")
                        .with_path(ws_a_path.join("crate-a"))
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-b").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-b")
                        .with_path(ws_a_path.join("crate-b"))
                        .with_dev_dependencies(vec![
                            Dependency::builder().with_name("crate-c").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-c")
                        .with_path(ws_a_path.join("crate-c"))
                        .with_build_dependencies(vec![
                            Dependency::builder().with_name("crate-a").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                ])
                .build()
                .unwrap(),
        );

        let mut builder = DependencyGraphBuilder::new(false, false, false);
        builder
            .build_intra_workspace_graph(&workspaces, None)
            .unwrap();

        assert_eq!(builder.graph.node_count(), 3);
        assert_eq!(builder.graph.edge_count(), 3); // One edge of each type

        // Verify different dependency types are present
        let edge_types: Vec<_> = builder
            .graph
            .edge_weights()
            .map(|edge| edge.dependency_type().clone())
            .collect();

        assert!(edge_types.contains(&DependencyType::Normal));
        assert!(edge_types.contains(&DependencyType::Dev));
        assert!(edge_types.contains(&DependencyType::Build));
    }

    #[test]
    fn test_intra_workspace_no_external_dependencies() {
        let mut workspaces = HashMap::new();

        // Create workspace with external dependencies that should be ignored
        let ws_a_path = PathBuf::from("/test/workspace-a");
        workspaces.insert(
            ws_a_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-a")
                .with_members(vec![
                    CrateMember::builder()
                        .with_name("crate-a")
                        .with_path("/test/workspace-a/crate-a")
                        .with_dependencies(vec![
                            Dependency::builder()
                                .with_name("crate-b") // Internal dependency
                                .build()
                                .unwrap(),
                            Dependency::builder()
                                .with_name("external-crate") // External dependency
                                // (should be ignored)
                                .build()
                                .unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-b")
                        .with_path("/test/workspace-a/crate-b")
                        .build()
                        .unwrap(),
                ])
                .build()
                .unwrap(),
        );

        let mut builder = DependencyGraphBuilder::new(false, false, false);
        builder
            .build_intra_workspace_graph(&workspaces, None)
            .unwrap();

        assert_eq!(builder.graph.node_count(), 2);
        assert_eq!(builder.graph.edge_count(), 1); // Only the internal dependency should create an edge

        // Verify the edge is between internal crates only
        let edge = builder.graph.edge_weights().next().unwrap();
        assert_eq!(edge.from_crate(), "crate-a");
        assert_eq!(edge.to_crate(), "crate-b");
    }

    #[test]
    fn test_intra_workspace_multiple_workspaces_isolation() {
        let mut workspaces = HashMap::new();

        // Create two workspaces, each with internal cycles
        let ws_a_path = PathBuf::from("/test/workspace-a");
        workspaces.insert(
            ws_a_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-a")
                .with_members(vec![
                    CrateMember::builder()
                        .with_name("crate-a1")
                        .with_path("/test/workspace-a/crate-a1")
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-a2").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-a2")
                        .with_path("/test/workspace-a/crate-a2")
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-a1").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                ])
                .build()
                .unwrap(),
        );

        let ws_b_path = PathBuf::from("/test/workspace-b");
        workspaces.insert(
            ws_b_path.clone(),
            WorkspaceInfo::builder()
                .with_name("workspace-b")
                .with_members(vec![
                    CrateMember::builder()
                        .with_name("crate-b1")
                        .with_path("/test/workspace-b/crate-b1")
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-b2").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                    CrateMember::builder()
                        .with_name("crate-b2")
                        .with_path("/test/workspace-b/crate-b2")
                        .with_dependencies(vec![
                            Dependency::builder().with_name("crate-b1").build().unwrap(),
                        ])
                        .build()
                        .unwrap(),
                ])
                .build()
                .unwrap(),
        );

        let mut builder = DependencyGraphBuilder::new(false, false, false);
        builder
            .build_intra_workspace_graph(&workspaces, None)
            .unwrap();

        // Should have 4 nodes (2 per workspace)
        assert_eq!(builder.graph.node_count(), 4);
        // Should have 4 edges (2 cycles, one in each workspace)
        assert_eq!(builder.graph.edge_count(), 4);

        // Verify nodes are properly isolated by workspace
        let node_names: Vec<String> = builder
            .graph
            .node_weights()
            .map(|n| n.name().to_string())
            .collect();
        assert!(node_names.contains(&"workspace-a/crate-a1".to_string()));
        assert!(node_names.contains(&"workspace-a/crate-a2".to_string()));
        assert!(node_names.contains(&"workspace-b/crate-b1".to_string()));
        assert!(node_names.contains(&"workspace-b/crate-b2".to_string()));
    }
}
