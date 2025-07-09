//! # Graph Construction and Rendering Module
//!
//! This module provides functionality for building and visualizing dependency
//! graphs. It supports multiple output formats and can highlight dependency
//! cycles.
//!
//! ## Components
//!
//! ### Graph Building
//! - **DependencyGraphBuilder**: Constructs the dependency graph from workspace
//!   analysis
//! - **WorkspaceNode**: Represents a workspace in the graph
//! - **DependencyEdge**: Represents a dependency relationship between crates
//!
//! ### Graph Rendering
//! - **GraphRenderer**: Renders graphs in various formats (DOT, Mermaid)
//! - Supports cycle highlighting and different visualization options
//!
//! ## Example
//!
//! ```
//! use cargo_ferris_wheel::common::ConfigBuilder;
//! use cargo_ferris_wheel::graph::{DependencyEdge, DependencyType, GraphRenderer, WorkspaceNode};
//! use petgraph::graph::DiGraph;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a simple dependency graph
//! let mut graph = DiGraph::new();
//!
//! // Add workspace nodes
//! let core = graph.add_node(
//!     WorkspaceNode::builder()
//!         .with_name("core".to_string())
//!         .with_crates(vec!["core-lib".to_string()])
//!         .build()
//!         .unwrap(),
//! );
//!
//! let app = graph.add_node(
//!     WorkspaceNode::builder()
//!         .with_name("app".to_string())
//!         .with_crates(vec!["app-main".to_string()])
//!         .build()
//!         .unwrap(),
//! );
//!
//! // Add a dependency edge
//! graph.add_edge(
//!     app,
//!     core,
//!     DependencyEdge::builder()
//!         .with_from_crate("app-main")
//!         .with_to_crate("core-lib")
//!         .with_dependency_type(DependencyType::Normal)
//!         .build()
//!         .unwrap(),
//! );
//!
//! // Render to DOT format
//! let renderer = GraphRenderer::new(true, true);
//! let mut output = Vec::new();
//! renderer.render_dot(&graph, &[], &mut output)?;
//!
//! let dot_output = String::from_utf8(output)?;
//! assert!(dot_output.contains("digraph"));
//! assert!(dot_output.contains("core"));
//! assert!(dot_output.contains("app"));
//! # Ok(())
//! # }
//! ```
//!
//! ## Output Formats
//!
//! - **DOT**: Graphviz format for detailed visualization
//! - **Mermaid**: Markdown-compatible diagrams for documentation

mod builder;
mod renderer;
mod types;

// Re-export main types and builders
pub use builder::DependencyGraphBuilder;
pub use renderer::GraphRenderer;
pub use types::{
    DependencyEdge, DependencyEdgeBuilder, DependencyType, WorkspaceNode, WorkspaceNodeBuilder,
};
