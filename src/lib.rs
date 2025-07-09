//! # Ferris Wheel - Detect Dependency Cycles in Rust Monorepos
//!
//! Ferris Wheel is a tool for detecting circular dependencies in Rust
//! workspaces. It analyzes Cargo workspace structures and identifies dependency
//! cycles that could lead to compilation issues or architectural problems.
//!
//! ## Main Components
//!
//! - **Analyzer**: Discovers and analyzes Rust workspaces and their
//!   dependencies
//! - **Detector**: Implements cycle detection algorithms (Tarjan's SCC)
//! - **Graph**: Builds and manages the dependency graph representation
//! - **Reports**: Generates human-readable and machine-readable reports
//!
//! ## Usage
//!
//! The library can be used programmatically:
//!
//! ```
//! use std::collections::HashMap;
//! use std::sync::Arc;
//!
//! use cargo_ferris_wheel::ConfigBuilder;
//! use cargo_ferris_wheel::analyzer::WorkspaceAnalyzer;
//! use cargo_ferris_wheel::detector::CycleDetector;
//! use cargo_ferris_wheel::graph::{
//!     DependencyEdge, DependencyGraphBuilder, DependencyType, WorkspaceNode,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a simple test scenario
//! let mut analyzer = WorkspaceAnalyzer::new();
//!
//! // For testing, we'll manually add some workspace info
//! // In real usage, you'd use analyzer.discover_workspaces()
//! let mut graph = petgraph::graph::DiGraph::new();
//!
//! // Add some test nodes
//! let ws1 = graph.add_node(
//!     WorkspaceNode::builder()
//!         .with_name("workspace-a".to_string())
//!         .with_crates(vec!["crate-a".to_string()])
//!         .build()
//!         .unwrap(),
//! );
//! let ws2 = graph.add_node(
//!     WorkspaceNode::builder()
//!         .with_name("workspace-b".to_string())
//!         .with_crates(vec!["crate-b".to_string()])
//!         .build()
//!         .unwrap(),
//! );
//!
//! // Add a dependency
//! graph.add_edge(
//!     ws1,
//!     ws2,
//!     DependencyEdge::builder()
//!         .with_from_crate("crate-a")
//!         .with_to_crate("crate-b")
//!         .with_dependency_type(DependencyType::Normal)
//!         .build()
//!         .unwrap(),
//! );
//!
//! // Detect cycles
//! let mut detector = CycleDetector::new();
//! detector.detect_cycles(&graph)?;
//!
//! assert!(!detector.has_cycles());
//! # Ok(())
//! # }
//! ```

// Private modules
mod common;
mod constants;
mod dependency_filter;
mod progress;
mod toml_parser;
mod utils;
mod workspace_discovery;

// Public modules
pub mod analyzer;
pub mod cli;
pub mod commands;
pub mod config;
pub mod core;
pub mod detector;
pub mod error;
pub mod executors;
pub mod graph;
pub mod reports;

// Re-export commonly used types
pub use crate::common::ConfigBuilder;
pub use crate::detector::{CycleEdge, WorkspaceCycle};
pub use crate::error::FerrisWheelError;
pub use crate::graph::{DependencyEdge, DependencyType, WorkspaceNode};

// Main entry point for the library
pub fn run() -> miette::Result<()> {
    use clap::Parser;

    use crate::cli::{CargoArgs, CargoCommand};
    use crate::commands::execute_command;

    let cargo_args = CargoArgs::parse();
    let CargoCommand::FerrisWheel(cli) = cargo_args.command;

    execute_command(cli.command)
}
