//! # Cycle Detection Module
//!
//! This module implements algorithms for detecting circular dependencies
//! in the workspace dependency graph.
//!
//! ## Algorithm
//!
//! We use Tarjan's Strongly Connected Components (SCC) algorithm to efficiently
//! find all cycles in the dependency graph. This algorithm has O(V + E) time
//! complexity where V is the number of vertices (workspaces) and E is the
//! number of edges (dependencies).
//!
//! ## Key Components
//!
//! - **CycleDetector**: Main detector that finds cycles using Tarjan's
//!   algorithm
//! - **WorkspaceCycle**: Represents a detected cycle with participating
//!   workspaces
//! - **CycleEdge**: Represents a dependency edge within a cycle
//!
//! ## Example
//!
//! ```
//! use ferris_wheel::detector::CycleDetector;
//! use ferris_wheel::graph::{DependencyEdge, DependencyType, WorkspaceNode};
//! use petgraph::graph::DiGraph;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a graph with a cycle
//! let mut graph = DiGraph::new();
//!
//! let a = graph.add_node(WorkspaceNode {
//!     name: "workspace-a".to_string(),
//!     crates: vec!["crate-a".to_string()],
//! });
//! let b = graph.add_node(WorkspaceNode {
//!     name: "workspace-b".to_string(),
//!     crates: vec!["crate-b".to_string()],
//! });
//!
//! // Create a cycle: A -> B -> A
//! graph.add_edge(
//!     a,
//!     b,
//!     DependencyEdge {
//!         from_crate: "crate-a".to_string(),
//!         to_crate: "crate-b".to_string(),
//!         dependency_type: DependencyType::Normal,
//!         target: None,
//!     },
//! );
//! graph.add_edge(
//!     b,
//!     a,
//!     DependencyEdge {
//!         from_crate: "crate-b".to_string(),
//!         to_crate: "crate-a".to_string(),
//!         dependency_type: DependencyType::Normal,
//!         target: None,
//!     },
//! );
//!
//! let mut detector = CycleDetector::new();
//! detector.detect_cycles(&graph)?;
//!
//! assert!(detector.has_cycles());
//! assert_eq!(detector.cycle_count(), 1);
//! # Ok(())
//! # }
//! ```

mod detector_impl;

pub use detector_impl::*;
