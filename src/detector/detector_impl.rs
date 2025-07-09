use std::collections::{HashMap, HashSet};

use miette::{Result, WrapErr};
use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::graph::{DependencyEdge, WorkspaceNode};

// Removed CycleSearchContext and related builder - no longer needed
// Now we collect all edges between workspaces in an SCC directly

/// Detector for finding dependency cycles in workspace graphs
///
/// Uses Tarjan's Strongly Connected Components algorithm to efficiently
/// find all cycles in the dependency graph.
pub struct CycleDetector {
    cycles: Vec<WorkspaceCycle>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceCycle {
    workspace_names: Vec<String>,
    edges: Vec<CycleEdge>,
    edges_by_direction: HashMap<(String, String), Vec<CycleEdge>>,
}

impl WorkspaceCycle {
    pub fn builder() -> WorkspaceCycleBuilder {
        WorkspaceCycleBuilder::new()
    }

    pub fn workspace_names(&self) -> &[String] {
        &self.workspace_names
    }

    pub fn edges(&self) -> &[CycleEdge] {
        &self.edges
    }

    pub fn edges_by_direction(&self) -> &HashMap<(String, String), Vec<CycleEdge>> {
        &self.edges_by_direction
    }
}

pub struct WorkspaceCycleBuilder {
    workspace_names: HashSet<String>,
    edges: Vec<CycleEdge>,
    edges_by_direction: HashMap<(String, String), Vec<CycleEdge>>,
}

impl Default for WorkspaceCycleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceCycleBuilder {
    pub fn new() -> Self {
        Self {
            workspace_names: HashSet::new(),
            edges: Vec::new(),
            edges_by_direction: HashMap::new(),
        }
    }

    pub fn add_edge(self) -> CycleEdgeBuilder<Self> {
        CycleEdgeBuilder::new(self)
    }

    pub fn with_workspace_names(mut self, names: Vec<String>) -> Self {
        self.workspace_names = names.into_iter().collect();
        self
    }

    fn add_edge_internal(&mut self, edge: CycleEdge) {
        // Add to workspace names
        self.workspace_names.insert(edge.from_workspace.clone());
        self.workspace_names.insert(edge.to_workspace.clone());

        // Add to edges_by_direction
        let direction = (edge.from_workspace.clone(), edge.to_workspace.clone());
        self.edges_by_direction
            .entry(direction)
            .or_default()
            .push(edge.clone());

        // Add to edges
        self.edges.push(edge);
    }

    pub fn build(self) -> WorkspaceCycle {
        let mut workspace_names: Vec<String> = self.workspace_names.into_iter().collect();
        workspace_names.sort();

        WorkspaceCycle {
            workspace_names,
            edges: self.edges,
            edges_by_direction: self.edges_by_direction,
        }
    }
}

pub struct CycleEdgeBuilder<T> {
    parent: T,
    from_workspace: Option<String>,
    to_workspace: Option<String>,
    from_crate: Option<String>,
    to_crate: Option<String>,
    dependency_type: Option<String>,
}

impl<T> CycleEdgeBuilder<T> {
    fn new(parent: T) -> Self {
        Self {
            parent,
            from_workspace: None,
            to_workspace: None,
            from_crate: None,
            to_crate: None,
            dependency_type: None,
        }
    }

    pub fn from_workspace(mut self, ws: &str) -> Self {
        self.from_workspace = Some(ws.to_string());
        self
    }

    pub fn to_workspace(mut self, ws: &str) -> Self {
        self.to_workspace = Some(ws.to_string());
        self
    }

    pub fn from_crate(mut self, cr: &str) -> Self {
        self.from_crate = Some(cr.to_string());
        self
    }

    pub fn to_crate(mut self, cr: &str) -> Self {
        self.to_crate = Some(cr.to_string());
        self
    }

    pub fn dependency_type(mut self, dt: &str) -> Self {
        self.dependency_type = Some(dt.to_string());
        self
    }
}

impl CycleEdgeBuilder<WorkspaceCycleBuilder> {
    pub fn add_edge(self) -> CycleEdgeBuilder<WorkspaceCycleBuilder> {
        let parent = self.build_and_add();
        CycleEdgeBuilder::new(parent)
    }

    pub fn build(self) -> WorkspaceCycle {
        let parent = self.build_and_add();
        parent.build()
    }

    fn build_and_add(mut self) -> WorkspaceCycleBuilder {
        let edge = CycleEdge {
            from_workspace: self.from_workspace.expect("from_workspace is required"),
            to_workspace: self.to_workspace.expect("to_workspace is required"),
            from_crate: self.from_crate.expect("from_crate is required"),
            to_crate: self.to_crate.expect("to_crate is required"),
            dependency_type: self.dependency_type.expect("dependency_type is required"),
        };
        self.parent.add_edge_internal(edge);
        self.parent
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleEdge {
    pub from_workspace: String,
    pub to_workspace: String,
    pub from_crate: String,
    pub to_crate: String,
    pub dependency_type: String,
}

impl Default for CycleDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl CycleDetector {
    /// Create a new cycle detector
    pub fn new() -> Self {
        Self { cycles: Vec::new() }
    }

    /// Detect all cycles in the dependency graph
    ///
    /// Uses Tarjan's algorithm to find strongly connected components,
    /// then identifies actual cycles within each component.
    pub fn detect_cycles(&mut self, graph: &DiGraph<WorkspaceNode, DependencyEdge>) -> Result<()> {
        // Use Tarjan's algorithm to find strongly connected components
        let sccs = tarjan_scc(graph);

        // Filter SCCs with more than one node (these contain cycles)
        for scc in sccs {
            if scc.len() > 1 {
                // Find all elementary cycles within this SCC
                self.find_all_cycles_in_scc(graph, scc)
                    .wrap_err("Failed to find cycles in SCC")?;
            }
        }

        Ok(())
    }

    fn find_all_cycles_in_scc(
        &mut self,
        graph: &DiGraph<WorkspaceNode, DependencyEdge>,
        scc: Vec<NodeIndex>,
    ) -> Result<()> {
        // For workspace cycles, we just need to know which workspaces form a cycle
        // and collect ALL edges between them

        if scc.len() < 2 {
            return Ok(());
        }

        // Get workspace names for the SCC
        let workspace_names: Vec<String> = scc.iter().map(|&idx| graph[idx].name.clone()).collect();

        // Create a builder for the cycle
        let mut builder = WorkspaceCycle::builder().with_workspace_names(workspace_names.clone());

        // Create a set for quick lookup
        let scc_set: HashSet<NodeIndex> = scc.iter().cloned().collect();

        let mut edge_count = 0;
        let mut edges_by_direction_check: HashMap<(String, String), bool> = HashMap::new();

        for &from_idx in &scc {
            let from_node = &graph[from_idx];

            for edge in graph.edges(from_idx) {
                let to_idx = edge.target();
                if scc_set.contains(&to_idx) && from_idx != to_idx {
                    let edge_data = edge.weight();
                    let to_node = &graph[to_idx];

                    // Track edge directions for 2-node cycle check
                    let direction = (from_node.name.clone(), to_node.name.clone());
                    edges_by_direction_check.insert(direction, true);

                    // Add edge using the internal method
                    let cycle_edge = CycleEdge {
                        from_workspace: from_node.name.clone(),
                        to_workspace: to_node.name.clone(),
                        from_crate: edge_data.from_crate.clone(),
                        to_crate: edge_data.to_crate.clone(),
                        dependency_type: format!("{:?}", edge_data.dependency_type),
                    };
                    builder.add_edge_internal(cycle_edge);
                    edge_count += 1;
                }
            }
        }

        // Only create a cycle if there are edges
        if edge_count > 0 {
            // For 2-node cycles, verify bidirectional dependencies exist
            if scc.len() == 2 {
                let ws1 = &workspace_names[0];
                let ws2 = &workspace_names[1];

                let has_forward =
                    edges_by_direction_check.contains_key(&(ws1.clone(), ws2.clone()));
                let has_backward =
                    edges_by_direction_check.contains_key(&(ws2.clone(), ws1.clone()));

                if has_forward && has_backward {
                    self.cycles.push(builder.build());
                }
            } else {
                // For larger SCCs, all nodes are mutually reachable
                self.cycles.push(builder.build());
            }
        }

        Ok(())
    }

    // Removed deduplicate_cycles - no longer needed with new approach

    /// Get all detected cycles
    pub fn cycles(&self) -> &[WorkspaceCycle] {
        &self.cycles
    }

    /// Check if any cycles were detected
    pub fn has_cycles(&self) -> bool {
        !self.cycles.is_empty()
    }

    /// Get the number of detected cycles
    pub fn cycle_count(&self) -> usize {
        self.cycles.len()
    }

    /// Add a cycle to the detector (used for filtered results)
    pub fn add_cycle(&mut self, cycle: WorkspaceCycle) {
        // The builder already ensures edges_by_direction is populated
        self.cycles.push(cycle);
    }
}

#[cfg(test)]
mod tests {
    use petgraph::graph::DiGraph;

    use super::*;
    use crate::common::ConfigBuilder;
    use crate::graph::{DependencyEdge, DependencyType, WorkspaceNode};

    #[test]
    fn test_no_cycles_in_linear_graph() {
        let mut graph = DiGraph::new();

        // Create a linear dependency: A -> B -> C
        let a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a".to_string()])
                .build()
                .unwrap(),
        );
        let b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b".to_string()])
                .build()
                .unwrap(),
        );
        let c = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-c".to_string())
                .with_crates(vec!["crate-c".to_string()])
                .build()
                .unwrap(),
        );

        graph.add_edge(
            a,
            b,
            DependencyEdge::builder()
                .with_from_crate("crate-a")
                .with_to_crate("crate-b")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            b,
            c,
            DependencyEdge::builder()
                .with_from_crate("crate-b")
                .with_to_crate("crate-c")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        assert_eq!(detector.cycle_count(), 0);
        assert!(!detector.has_cycles());
    }

    #[test]
    fn test_simple_two_node_cycle() {
        let mut graph = DiGraph::new();

        // Create a simple cycle: A <-> B
        let a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a".to_string()])
                .build()
                .unwrap(),
        );
        let b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b".to_string()])
                .build()
                .unwrap(),
        );

        graph.add_edge(
            a,
            b,
            DependencyEdge::builder()
                .with_from_crate("crate-a")
                .with_to_crate("crate-b")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            b,
            a,
            DependencyEdge::builder()
                .with_from_crate("crate-b")
                .with_to_crate("crate-a")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        assert_eq!(detector.cycle_count(), 1);
        assert!(detector.has_cycles());

        let cycle = &detector.cycles()[0];
        assert_eq!(cycle.edges().len(), 2);
        assert_eq!(cycle.workspace_names().len(), 2);

        // Check edges are grouped by direction
        assert_eq!(cycle.edges_by_direction().len(), 2);
        assert!(
            cycle
                .edges_by_direction()
                .contains_key(&("workspace-a".to_string(), "workspace-b".to_string()))
        );
        assert!(
            cycle
                .edges_by_direction()
                .contains_key(&("workspace-b".to_string(), "workspace-a".to_string()))
        );
    }

    #[test]
    fn test_three_node_cycle() {
        let mut graph = DiGraph::new();

        // Create a three-node cycle: A -> B -> C -> A
        let a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a".to_string()])
                .build()
                .unwrap(),
        );
        let b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b".to_string()])
                .build()
                .unwrap(),
        );
        let c = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-c".to_string())
                .with_crates(vec!["crate-c".to_string()])
                .build()
                .unwrap(),
        );

        graph.add_edge(
            a,
            b,
            DependencyEdge::builder()
                .with_from_crate("crate-a")
                .with_to_crate("crate-b")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            b,
            c,
            DependencyEdge::builder()
                .with_from_crate("crate-b")
                .with_to_crate("crate-c")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            c,
            a,
            DependencyEdge::builder()
                .with_from_crate("crate-c")
                .with_to_crate("crate-a")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        assert_eq!(detector.cycle_count(), 1);
        assert!(detector.has_cycles());

        let cycle = &detector.cycles()[0];
        assert_eq!(cycle.edges().len(), 3);
        assert_eq!(cycle.workspace_names().len(), 3);
        assert_eq!(cycle.edges_by_direction().len(), 3);
    }

    #[test]
    fn test_workspace_cycle_with_multiple_edges() {
        let mut graph = DiGraph::new();

        // Create workspaces with multiple crates
        let ws_a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec![
                    "crate-a1".to_string(),
                    "crate-a2".to_string(),
                    "crate-a3".to_string(),
                ])
                .build()
                .unwrap(),
        );
        let ws_b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b1".to_string(), "crate-b2".to_string()])
                .build()
                .unwrap(),
        );

        // Add multiple edges from A to B
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a1")
                .with_to_crate("crate-b1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a2")
                .with_to_crate("crate-b1")
                .with_dependency_type(DependencyType::Dev)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a3")
                .with_to_crate("crate-b2")
                .with_dependency_type(DependencyType::Build)
                .build()
                .unwrap(),
        );

        // Add edges from B to A
        graph.add_edge(
            ws_b,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-b1")
                .with_to_crate("crate-a1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-b2")
                .with_to_crate("crate-a2")
                .with_dependency_type(DependencyType::Dev)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        assert_eq!(
            detector.cycle_count(),
            1,
            "Should find exactly one workspace cycle"
        );

        let cycle = &detector.cycles()[0];
        assert_eq!(cycle.edges().len(), 5, "Should have all 5 edges");
        assert_eq!(cycle.workspace_names().len(), 2);

        // Check edge grouping
        assert_eq!(cycle.edges_by_direction().len(), 2);

        let a_to_b_edges = cycle
            .edges_by_direction()
            .get(&("workspace-a".to_string(), "workspace-b".to_string()))
            .unwrap();
        assert_eq!(a_to_b_edges.len(), 3, "Should have 3 edges from A to B");

        let b_to_a_edges = cycle
            .edges_by_direction()
            .get(&("workspace-b".to_string(), "workspace-a".to_string()))
            .unwrap();
        assert_eq!(b_to_a_edges.len(), 2, "Should have 2 edges from B to A");

        // Verify edge types are preserved
        let edge_types: Vec<String> = cycle
            .edges()
            .iter()
            .map(|e| e.dependency_type.clone())
            .collect();
        assert!(edge_types.contains(&"Normal".to_string()));
        assert!(edge_types.contains(&"Dev".to_string()));
        assert!(edge_types.contains(&"Build".to_string()));
    }

    #[test]
    fn test_multiple_cycles_in_same_scc() {
        let mut graph = DiGraph::new();

        // Create a fully connected graph with 3 nodes (multiple cycles)
        // This should have cycles: A->B->A, B->C->B, A->C->A, A->B->C->A
        let a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a".to_string()])
                .build()
                .unwrap(),
        );
        let b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b".to_string()])
                .build()
                .unwrap(),
        );
        let c = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-c".to_string())
                .with_crates(vec!["crate-c".to_string()])
                .build()
                .unwrap(),
        );

        graph.add_edge(
            a,
            b,
            DependencyEdge::builder()
                .with_from_crate("crate-a")
                .with_to_crate("crate-b")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            b,
            c,
            DependencyEdge::builder()
                .with_from_crate("crate-b")
                .with_to_crate("crate-c")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            c,
            a,
            DependencyEdge::builder()
                .with_from_crate("crate-c")
                .with_to_crate("crate-a")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            b,
            a,
            DependencyEdge::builder()
                .with_from_crate("crate-b")
                .with_to_crate("crate-a")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            c,
            b,
            DependencyEdge::builder()
                .with_from_crate("crate-c")
                .with_to_crate("crate-b")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            a,
            c,
            DependencyEdge::builder()
                .with_from_crate("crate-a")
                .with_to_crate("crate-c")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // With the new approach, a fully connected graph forms one workspace cycle
        assert_eq!(detector.cycle_count(), 1);

        let cycle = &detector.cycles()[0];
        assert_eq!(
            cycle.workspace_names().len(),
            3,
            "Should contain all 3 workspaces"
        );
        assert_eq!(cycle.edges().len(), 6, "Should have all 6 edges");
        assert!(detector.has_cycles());
    }

    #[test]
    fn test_dev_dependency_cycle() {
        let mut graph = DiGraph::new();

        // Create a cycle with mixed dependency types
        // nodes -> core (normal), core -> nodes (dev)
        let nodes = graph.add_node(
            WorkspaceNode::builder()
                .with_name("nodes".to_string())
                .with_crates(vec!["sequencer-node".to_string()])
                .build()
                .unwrap(),
        );
        let core = graph.add_node(
            WorkspaceNode::builder()
                .with_name("core".to_string())
                .with_crates(vec!["testing-utils".to_string()])
                .build()
                .unwrap(),
        );

        graph.add_edge(
            nodes,
            core,
            DependencyEdge::builder()
                .with_from_crate("sequencer-node")
                .with_to_crate("testing-utils")
                .with_dependency_type(DependencyType::Dev)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            core,
            nodes,
            DependencyEdge::builder()
                .with_from_crate("testing-utils")
                .with_to_crate("sequencer-node")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        assert_eq!(detector.cycle_count(), 1);

        let cycle = &detector.cycles()[0];
        assert_eq!(cycle.edges().len(), 2);

        // Verify the dependency types are preserved
        let has_dev_dep = cycle.edges().iter().any(|e| e.dependency_type == "Dev");
        let has_normal_dep = cycle.edges().iter().any(|e| e.dependency_type == "Normal");
        assert!(has_dev_dep);
        assert!(has_normal_dep);
    }

    #[test]
    fn test_multiple_edges_between_same_workspaces() {
        let mut graph = DiGraph::new();

        // Create multiple edges between the same two workspaces
        // (different crates creating dependencies)
        let a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a1".to_string(), "crate-a2".to_string()])
                .build()
                .unwrap(),
        );
        let b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b1".to_string(), "crate-b2".to_string()])
                .build()
                .unwrap(),
        );

        graph.add_edge(
            a,
            b,
            DependencyEdge::builder()
                .with_from_crate("crate-a1")
                .with_to_crate("crate-b1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            a,
            b,
            DependencyEdge::builder()
                .with_from_crate("crate-a2")
                .with_to_crate("crate-b2")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            b,
            a,
            DependencyEdge::builder()
                .with_from_crate("crate-b1")
                .with_to_crate("crate-a1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // With the new approach, this creates one workspace cycle
        assert_eq!(detector.cycle_count(), 1);

        let cycle = &detector.cycles()[0];
        assert_eq!(cycle.edges().len(), 3, "Should have all 3 edges");
        assert_eq!(
            cycle.edges_by_direction().len(),
            2,
            "Should have 2 directions"
        );
    }

    #[test]
    fn test_complex_multi_workspace_scenario() {
        let mut graph = DiGraph::new();

        // Recreate the scenario from the actual codebase:
        // nodes/token-indexer -> sdk/spl-token-metadata-api
        // sdk/program-test-internal -> core/standalone-svm
        // core/sequencer-testing-utils -> nodes/sequencer-node
        // nodes/sequencer-node -> core/sequencer-testing-utils (dev)

        let nodes = graph.add_node(
            WorkspaceNode::builder()
                .with_name("nodes".to_string())
                .with_crates(vec![
                    "token-indexer".to_string(),
                    "sequencer-node".to_string(),
                ])
                .build()
                .unwrap(),
        );
        let sdk = graph.add_node(
            WorkspaceNode::builder()
                .with_name("sdk".to_string())
                .with_crates(vec![
                    "spl-token-metadata-api".to_string(),
                    "program-test-internal".to_string(),
                ])
                .build()
                .unwrap(),
        );
        let core = graph.add_node(
            WorkspaceNode::builder()
                .with_name("core".to_string())
                .with_crates(vec![
                    "standalone-svm".to_string(),
                    "sequencer-testing-utils".to_string(),
                ])
                .build()
                .unwrap(),
        );

        // Add the edges
        graph.add_edge(
            nodes,
            sdk,
            DependencyEdge::builder()
                .with_from_crate("token-indexer")
                .with_to_crate("spl-token-metadata-api")
                .with_dependency_type(DependencyType::Normal)
                .with_target(None)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            sdk,
            core,
            DependencyEdge::builder()
                .with_from_crate("program-test-internal")
                .with_to_crate("standalone-svm")
                .with_dependency_type(DependencyType::Normal)
                .with_target(None)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            core,
            nodes,
            DependencyEdge::builder()
                .with_from_crate("sequencer-testing-utils")
                .with_to_crate("sequencer-node")
                .with_dependency_type(DependencyType::Normal)
                .with_target(None)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            nodes,
            core,
            DependencyEdge::builder()
                .with_from_crate("sequencer-node")
                .with_to_crate("sequencer-testing-utils")
                .with_dependency_type(DependencyType::Dev)
                .with_target(None)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // With the new approach, all three workspaces form one SCC
        // so we get one workspace cycle containing all three
        assert_eq!(detector.cycle_count(), 1);

        let cycle = &detector.cycles()[0];
        assert_eq!(
            cycle.workspace_names().len(),
            3,
            "Should contain all 3 workspaces"
        );
        assert_eq!(cycle.edges().len(), 4, "Should have all 4 edges");

        // Verify edge grouping
        assert!(
            cycle
                .edges_by_direction()
                .contains_key(&("nodes".to_string(), "sdk".to_string()))
        );
        assert!(
            cycle
                .edges_by_direction()
                .contains_key(&("sdk".to_string(), "core".to_string()))
        );
        assert!(
            cycle
                .edges_by_direction()
                .contains_key(&("core".to_string(), "nodes".to_string()))
        );
        assert!(
            cycle
                .edges_by_direction()
                .contains_key(&("nodes".to_string(), "core".to_string()))
        );
    }

    #[test]
    fn test_direct_bidirectional_cycle_is_found() {
        let mut graph = DiGraph::new();

        // Specific test for the nodes <-> core cycle that should be detected
        let nodes = graph.add_node(
            WorkspaceNode::builder()
                .with_name("nodes".to_string())
                .with_crates(vec!["atlas-sequencer-node".to_string()])
                .build()
                .unwrap(),
        );
        let core = graph.add_node(
            WorkspaceNode::builder()
                .with_name("core".to_string())
                .with_crates(vec!["atlas-sequencer-testing-utils".to_string()])
                .build()
                .unwrap(),
        );

        // nodes/atlas-sequencer-node -> core/atlas-sequencer-testing-utils (dev
        // dependency)
        graph.add_edge(
            nodes,
            core,
            DependencyEdge::builder()
                .with_from_crate("atlas-sequencer-node")
                .with_to_crate("atlas-sequencer-testing-utils")
                .with_dependency_type(DependencyType::Dev)
                .with_target(None)
                .build()
                .unwrap(),
        );
        // core/atlas-sequencer-testing-utils -> nodes/atlas-sequencer-node (normal
        // dependency)
        graph.add_edge(
            core,
            nodes,
            DependencyEdge::builder()
                .with_from_crate("atlas-sequencer-testing-utils")
                .with_to_crate("atlas-sequencer-node")
                .with_dependency_type(DependencyType::Normal)
                .with_target(None)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // Should find the bidirectional cycle (might show it as 1 or 2 cycles depending
        // on deduplication)
        assert!(
            detector.cycle_count() >= 1,
            "Should find at least one cycle"
        );

        let cycle = &detector.cycles()[0];
        assert_eq!(
            cycle.workspace_names.len(),
            2,
            "Cycle should contain 2 workspaces"
        );
        assert_eq!(cycle.edges().len(), 2, "Cycle should have 2 edges");

        // Verify the cycle contains both workspaces
        assert!(cycle.workspace_names().contains(&"nodes".to_string()));
        assert!(cycle.workspace_names().contains(&"core".to_string()));

        // Verify both edges are present
        let edge_pairs: Vec<(String, String)> = cycle
            .edges()
            .iter()
            .map(|e| (e.from_workspace.clone(), e.to_workspace.clone()))
            .collect();

        assert!(edge_pairs.contains(&("nodes".to_string(), "core".to_string())));
        assert!(edge_pairs.contains(&("core".to_string(), "nodes".to_string())));

        // Print the cycle for debugging
        eprintln!("\nDetected cycle:");
        for edge in cycle.edges() {
            eprintln!(
                "  {} -> {} ({})",
                edge.from_workspace, edge.to_workspace, edge.dependency_type
            );
        }
    }

    #[test]
    fn test_inter_workspace_complex_cycles() {
        let mut graph = DiGraph::new();

        // Create a complex scenario with multiple cycles between different workspaces
        let ws_a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a1".to_string(), "crate-a2".to_string()])
                .build()
                .unwrap(),
        );
        let ws_b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b1".to_string(), "crate-b2".to_string()])
                .build()
                .unwrap(),
        );
        let ws_c = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-c".to_string())
                .with_crates(vec!["crate-c1".to_string(), "crate-c2".to_string()])
                .build()
                .unwrap(),
        );
        let ws_d = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-d".to_string())
                .with_crates(vec!["crate-d1".to_string()])
                .build()
                .unwrap(),
        );

        // Create multiple cycles:
        // 1. A -> B -> A (2-node cycle)
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a1")
                .with_to_crate("crate-b1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-b1")
                .with_to_crate("crate-a1")
                .with_dependency_type(DependencyType::Dev)
                .build()
                .unwrap(),
        );

        // 2. A -> C -> A (another 2-node cycle)
        graph.add_edge(
            ws_a,
            ws_c,
            DependencyEdge::builder()
                .with_from_crate("crate-a2")
                .with_to_crate("crate-c1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_c,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-c1")
                .with_to_crate("crate-a2")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        // 3. B -> C -> D -> B (3-node cycle)
        graph.add_edge(
            ws_b,
            ws_c,
            DependencyEdge::builder()
                .with_from_crate("crate-b2")
                .with_to_crate("crate-c2")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_c,
            ws_d,
            DependencyEdge::builder()
                .with_from_crate("crate-c2")
                .with_to_crate("crate-d1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_d,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-d1")
                .with_to_crate("crate-b2")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // All workspaces are interconnected, forming one SCC
        assert_eq!(detector.cycle_count(), 1, "Should find one workspace cycle");

        let cycle = &detector.cycles()[0];
        assert_eq!(
            cycle.workspace_names().len(),
            4,
            "Should contain all 4 workspaces"
        );
        // We have 7 edges: A→B, B→A, A→C, C→A, B→C, C→D, D→B
        assert_eq!(cycle.edges().len(), 7, "Should have all 7 edges");

        // Verify edge directions
        assert_eq!(
            cycle.edges_by_direction().len(),
            7,
            "Should have 7 unique directions"
        );
    }

    #[test]
    fn test_mixed_dependency_types_cycles() {
        let mut graph = DiGraph::new();

        // Test cycles involving different dependency types
        let ws_a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a".to_string()])
                .build()
                .unwrap(),
        );
        let ws_b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b".to_string()])
                .build()
                .unwrap(),
        );
        let ws_c = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-c".to_string())
                .with_crates(vec!["crate-c".to_string()])
                .build()
                .unwrap(),
        );

        // Create cycle with mixed dependency types: A -normal-> B -dev-> C -build-> A
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a")
                .with_to_crate("crate-b")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_c,
            DependencyEdge::builder()
                .with_from_crate("crate-b")
                .with_to_crate("crate-c")
                .with_dependency_type(DependencyType::Dev)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_c,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-c")
                .with_to_crate("crate-a")
                .with_dependency_type(DependencyType::Build)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        assert_eq!(detector.cycle_count(), 1, "Should find exactly one cycle");

        let cycle = &detector.cycles()[0];
        assert_eq!(cycle.edges().len(), 3, "Cycle should have 3 edges");

        // Verify all dependency types are present
        let dep_types: Vec<String> = cycle
            .edges()
            .iter()
            .map(|e| e.dependency_type.clone())
            .collect();

        assert!(dep_types.contains(&"Normal".to_string()));
        assert!(dep_types.contains(&"Dev".to_string()));
        assert!(dep_types.contains(&"Build".to_string()));
    }

    #[test]
    fn test_self_referencing_workspace() {
        let mut graph = DiGraph::new();

        // Test a workspace that depends on itself (should not create a cycle at
        // workspace level)
        let ws_a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a1".to_string(), "crate-a2".to_string()])
                .build()
                .unwrap(),
        );

        // This should not create a workspace-level cycle since it's within the same
        // workspace
        graph.add_edge(
            ws_a,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-a1")
                .with_to_crate("crate-a2")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // Should not find any cycles for inter-workspace analysis
        assert_eq!(
            detector.cycle_count(),
            0,
            "Self-referencing workspace should not create inter-workspace cycles"
        );
    }

    #[test]
    fn test_parallel_cycles_between_same_workspaces() {
        let mut graph = DiGraph::new();

        // Test multiple independent cycles between the same pair of workspaces
        let ws_a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec![
                    "crate-a1".to_string(),
                    "crate-a2".to_string(),
                    "crate-a3".to_string(),
                ])
                .build()
                .unwrap(),
        );
        let ws_b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec![
                    "crate-b1".to_string(),
                    "crate-b2".to_string(),
                    "crate-b3".to_string(),
                ])
                .build()
                .unwrap(),
        );

        // Create multiple independent cycles between A and B:
        // Cycle 1: a1 -> b1 -> a1
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a1")
                .with_to_crate("crate-b1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-b1")
                .with_to_crate("crate-a1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        // Cycle 2: a2 -> b2 -> a2
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a2")
                .with_to_crate("crate-b2")
                .with_dependency_type(DependencyType::Dev)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-b2")
                .with_to_crate("crate-a2")
                .with_dependency_type(DependencyType::Dev)
                .build()
                .unwrap(),
        );

        // Cycle 3: a3 -> b3 -> a3
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a3")
                .with_to_crate("crate-b3")
                .with_dependency_type(DependencyType::Build)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-b3")
                .with_to_crate("crate-a3")
                .with_dependency_type(DependencyType::Build)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // With the new approach, multiple edges between the same two workspaces
        // form a single workspace cycle
        assert_eq!(detector.cycle_count(), 1, "Should find one workspace cycle");

        let cycle = &detector.cycles()[0];
        assert_eq!(cycle.workspace_names().len(), 2, "Should be a 2-node cycle");
        assert_eq!(cycle.edges().len(), 6, "Should have all 6 edges");

        // Check edge grouping
        let a_to_b = cycle
            .edges_by_direction()
            .get(&("workspace-a".to_string(), "workspace-b".to_string()))
            .unwrap();
        assert_eq!(a_to_b.len(), 3, "Should have 3 edges from A to B");

        let b_to_a = cycle
            .edges_by_direction()
            .get(&("workspace-b".to_string(), "workspace-a".to_string()))
            .unwrap();
        assert_eq!(b_to_a.len(), 3, "Should have 3 edges from B to A");
    }

    #[test]
    fn test_transitive_cycle_detection() {
        let mut graph = DiGraph::new();

        // Test transitive cycles: A -> B -> C -> D -> A
        let ws_a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a".to_string()])
                .build()
                .unwrap(),
        );
        let ws_b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b".to_string()])
                .build()
                .unwrap(),
        );
        let ws_c = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-c".to_string())
                .with_crates(vec!["crate-c".to_string()])
                .build()
                .unwrap(),
        );
        let ws_d = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-d".to_string())
                .with_crates(vec!["crate-d".to_string()])
                .build()
                .unwrap(),
        );

        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a")
                .with_to_crate("crate-b")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_c,
            DependencyEdge::builder()
                .with_from_crate("crate-b")
                .with_to_crate("crate-c")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_c,
            ws_d,
            DependencyEdge::builder()
                .with_from_crate("crate-c")
                .with_to_crate("crate-d")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_d,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-d")
                .with_to_crate("crate-a")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        assert_eq!(
            detector.cycle_count(),
            1,
            "Should find exactly one 4-node cycle"
        );

        let cycle = &detector.cycles()[0];
        assert_eq!(
            cycle.workspace_names.len(),
            4,
            "Cycle should contain 4 workspaces"
        );
        assert_eq!(cycle.edges().len(), 4, "Cycle should have 4 edges");

        // Verify all workspaces are in the cycle
        let workspace_names = cycle.workspace_names();
        assert!(workspace_names.contains(&"workspace-a".to_string()));
        assert!(workspace_names.contains(&"workspace-b".to_string()));
        assert!(workspace_names.contains(&"workspace-c".to_string()));
        assert!(workspace_names.contains(&"workspace-d".to_string()));
    }

    #[test]
    fn test_overlapping_cycles_shared_nodes() {
        let mut graph = DiGraph::new();

        // Test scenario where multiple cycles share common workspaces
        let ws_a = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-a".to_string())
                .with_crates(vec!["crate-a1".to_string(), "crate-a2".to_string()])
                .build()
                .unwrap(),
        );
        let ws_b = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-b".to_string())
                .with_crates(vec!["crate-b1".to_string(), "crate-b2".to_string()])
                .build()
                .unwrap(),
        );
        let ws_c = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-c".to_string())
                .with_crates(vec!["crate-c".to_string()])
                .build()
                .unwrap(),
        );
        let ws_d = graph.add_node(
            WorkspaceNode::builder()
                .with_name("workspace-d".to_string())
                .with_crates(vec!["crate-d".to_string()])
                .build()
                .unwrap(),
        );

        // Create overlapping cycles:
        // Cycle 1: A -> B -> A (shares A,B with cycle 2)
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a1")
                .with_to_crate("crate-b1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-b1")
                .with_to_crate("crate-a1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        // Cycle 2: A -> B -> C -> A (shares A,B with cycle 1)
        graph.add_edge(
            ws_a,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-a2")
                .with_to_crate("crate-b2")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_b,
            ws_c,
            DependencyEdge::builder()
                .with_from_crate("crate-b2")
                .with_to_crate("crate-c")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_c,
            ws_a,
            DependencyEdge::builder()
                .with_from_crate("crate-c")
                .with_to_crate("crate-a2")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        // Cycle 3: B -> C -> D -> B (shares B,C with cycle 2)
        graph.add_edge(
            ws_b,
            ws_c,
            DependencyEdge::builder()
                .with_from_crate("crate-b1")
                .with_to_crate("crate-c")
                .with_dependency_type(DependencyType::Dev)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_c,
            ws_d,
            DependencyEdge::builder()
                .with_from_crate("crate-c")
                .with_to_crate("crate-d")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
        graph.add_edge(
            ws_d,
            ws_b,
            DependencyEdge::builder()
                .with_from_crate("crate-d")
                .with_to_crate("crate-b1")
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // All workspaces are interconnected through the overlapping cycles
        assert_eq!(detector.cycle_count(), 1, "Should find one workspace cycle");

        let cycle = &detector.cycles()[0];
        assert_eq!(
            cycle.workspace_names().len(),
            4,
            "Should contain all 4 workspaces"
        );
        // We have 8 edges: A→B(2 edges), B→A, B→C(2 edges), C→A, C→D, D→B
        assert_eq!(cycle.edges().len(), 8, "Should have all 8 edges");

        // Verify the edges are properly grouped
        // We have 6 unique directions (B→C has 2 edges in same direction)
        assert_eq!(
            cycle.edges_by_direction().len(),
            6,
            "Should have 6 unique directions"
        );

        // Check that B→C has 2 edges
        let b_to_c = cycle
            .edges_by_direction()
            .get(&("workspace-b".to_string(), "workspace-c".to_string()))
            .unwrap();
        assert_eq!(b_to_c.len(), 2, "Should have 2 edges from B to C");
    }

    #[test]
    fn test_large_complex_dependency_graph() {
        let mut graph = DiGraph::new();

        // Create a larger graph with 6 workspaces and complex interdependencies
        let workspaces: Vec<NodeIndex> = (0..6)
            .map(|i| {
                let ws_name = format!("workspace-{i}");
                let crate_names = [format!("crate-{i}-1"), format!("crate-{i}-2")];
                graph.add_node(
                    WorkspaceNode::builder()
                        .with_name(ws_name)
                        .with_crates(crate_names.to_vec())
                        .build()
                        .unwrap(),
                )
            })
            .collect();

        // Create a complex web of dependencies
        // Each workspace depends on the next two workspaces (modulo 6)
        for i in 0..6 {
            let from_ws = workspaces[i];
            let to_ws1 = workspaces[(i + 1) % 6];
            let to_ws2 = workspaces[(i + 2) % 6];

            graph.add_edge(
                from_ws,
                to_ws1,
                DependencyEdge::builder()
                    .with_from_crate(&format!("crate-{i}-1"))
                    .with_to_crate(&format!("crate-{}-1", (i + 1) % 6))
                    .with_dependency_type(DependencyType::Normal)
                    .with_target(None)
                    .build()
                    .unwrap(),
            );

            graph.add_edge(
                from_ws,
                to_ws2,
                DependencyEdge::builder()
                    .with_from_crate(&format!("crate-{i}-2"))
                    .with_to_crate(&format!("crate-{}-1", (i + 2) % 6))
                    .with_dependency_type(DependencyType::Dev)
                    .with_target(None)
                    .build()
                    .unwrap(),
            );
        }

        let mut detector = CycleDetector::new();
        detector.detect_cycles(&graph).unwrap();

        // With each workspace depending on the next two (modulo 6),
        // all workspaces are mutually reachable, forming one large SCC
        assert_eq!(detector.cycle_count(), 1, "Should find one workspace cycle");

        let cycle = &detector.cycles()[0];
        assert_eq!(
            cycle.workspace_names().len(),
            6,
            "Should contain all 6 workspaces"
        );
        assert_eq!(
            cycle.edges().len(),
            12,
            "Should have all 12 edges (2 per workspace)"
        );

        // Verify the cycle is well-formed
        assert!(!cycle.edges().is_empty(), "Cycle should not be empty");
        assert!(
            !cycle.workspace_names().is_empty(),
            "Cycle should have workspace names"
        );
        assert_eq!(
            cycle.edges_by_direction().len(),
            12,
            "Should have 12 unique directions"
        );
    }
}
