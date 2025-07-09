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
//! ### Real-World Example: Analyzing a Rust Monorepo
//!
//! ```no_run
//! use std::path::PathBuf;
//!
//! use cargo_ferris_wheel::analyzer::WorkspaceAnalyzer;
//! use cargo_ferris_wheel::detector::CycleDetector;
//! use cargo_ferris_wheel::graph::DependencyGraphBuilder;
//! use cargo_ferris_wheel::reports::{HumanReportGenerator, JsonReportGenerator, ReportGenerator};
//! use miette::IntoDiagnostic;
//!
//! # fn main() -> miette::Result<()> {
//! // Step 1: Discover all workspaces in your monorepo
//! let mut analyzer = WorkspaceAnalyzer::new();
//! let repo_root = PathBuf::from("/path/to/your/monorepo");
//! analyzer.discover_workspaces(&[repo_root], None)?;
//!
//! println!("Found {} workspaces", analyzer.workspaces().len());
//!
//! // Step 2: Build the dependency graph
//! let mut graph_builder = DependencyGraphBuilder::new(
//!     false, // include dev dependencies
//!     false, // include build dependencies
//!     false, // include target-specific dependencies
//! );
//!
//! graph_builder.build_cross_workspace_graph(
//!     analyzer.workspaces(),
//!     analyzer.crate_to_workspace(),
//!     None, // no progress reporter
//! )?;
//!
//! // Step 3: Detect circular dependencies
//! let mut detector = CycleDetector::new();
//! detector.detect_cycles(graph_builder.graph())?;
//!
//! // Step 4: Generate reports
//! if detector.has_cycles() {
//!     println!(
//!         "⚠️  Found {} circular dependencies!",
//!         detector.cycle_count()
//!     );
//!
//!     // Human-readable report for console output
//!     let human_report = HumanReportGenerator::new(Some(5)); // show max 5 cycles
//!     println!("{}", human_report.generate_report(&detector)?);
//!
//!     // JSON report for programmatic processing
//!     let json_report = JsonReportGenerator::new();
//!     let json_output = json_report.generate_report(&detector)?;
//!     std::fs::write("cycles.json", json_output).into_diagnostic()?;
//! } else {
//!     println!("✅ No circular dependencies found!");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Example: Visualizing the Dependency Graph
//!
//! ```no_run
//! use std::io::Write;
//!
//! use cargo_ferris_wheel::graph::GraphRenderer;
//! use miette::IntoDiagnostic;
//! # use std::path::PathBuf;
//! # use cargo_ferris_wheel::{
//! #     analyzer::WorkspaceAnalyzer,
//! #     detector::CycleDetector,
//! #     graph::DependencyGraphBuilder,
//! # };
//!
//! # fn main() -> miette::Result<()> {
//! # let mut analyzer = WorkspaceAnalyzer::new();
//! # analyzer.discover_workspaces(&[PathBuf::from(".")], None)?;
//! # let mut graph_builder = DependencyGraphBuilder::new(false, false, false);
//! # graph_builder.build_cross_workspace_graph(
//! #     analyzer.workspaces(),
//! #     analyzer.crate_to_workspace(),
//! #     None,
//! # )?;
//! # let mut detector = CycleDetector::new();
//! # detector.detect_cycles(graph_builder.graph())?;
//! // Create a visual representation of your dependency graph
//! let renderer = GraphRenderer::new(
//!     true,  // highlight cycles
//!     false, // don't show individual crate details
//! );
//!
//! // Generate a Mermaid diagram (great for documentation)
//! let mut mermaid_output = Vec::new();
//! renderer.render_mermaid(
//!     graph_builder.graph(),
//!     detector.cycles(),
//!     &mut mermaid_output,
//! )?;
//!
//! std::fs::write("dependencies.mmd", mermaid_output).into_diagnostic()?;
//!
//! // Or generate a DOT file for Graphviz
//! let mut dot_output = Vec::new();
//! renderer.render_dot(graph_builder.graph(), detector.cycles(), &mut dot_output)?;
//!
//! std::fs::write("dependencies.dot", dot_output).into_diagnostic()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Example: Filtering Dependencies
//!
//! ```no_run
//! # use std::path::PathBuf;
//! # use cargo_ferris_wheel::{
//! #     analyzer::WorkspaceAnalyzer,
//! #     detector::CycleDetector,
//! #     graph::DependencyGraphBuilder,
//! # };
//! # fn main() -> miette::Result<()> {
//! # let mut analyzer = WorkspaceAnalyzer::new();
//! # analyzer.discover_workspaces(&[PathBuf::from(".")], None)?;
//! // Check only production dependencies (exclude dev and build deps)
//! let mut graph_builder = DependencyGraphBuilder::new(
//!     true,  // exclude dev dependencies
//!     true,  // exclude build dependencies
//!     false, // include target-specific dependencies
//! );
//!
//! graph_builder.build_cross_workspace_graph(
//!     analyzer.workspaces(),
//!     analyzer.crate_to_workspace(),
//!     None,
//! )?;
//!
//! let mut detector = CycleDetector::new();
//! detector.detect_cycles(graph_builder.graph())?;
//!
//! println!("Production dependency cycles: {}", detector.cycle_count());
//! # Ok(())
//! # }
//! ```
//!
//! ### Example: Analyzing Specific Workspaces
//!
//! ```no_run
//! # use std::path::PathBuf;
//! # use cargo_ferris_wheel::{
//! #     analyzer::WorkspaceAnalyzer,
//! #     detector::{CycleDetector, WorkspaceCycle},
//! #     graph::DependencyGraphBuilder,
//! # };
//! # fn main() -> miette::Result<()> {
//! # let mut analyzer = WorkspaceAnalyzer::new();
//! # analyzer.discover_workspaces(&[PathBuf::from(".")], None)?;
//! # let mut graph_builder = DependencyGraphBuilder::new(false, false, false);
//! # graph_builder.build_cross_workspace_graph(
//! #     analyzer.workspaces(),
//! #     analyzer.crate_to_workspace(),
//! #     None,
//! # )?;
//! # let mut detector = CycleDetector::new();
//! # detector.detect_cycles(graph_builder.graph())?;
//! // Find cycles involving a specific workspace
//! let target_workspace = "backend-core";
//!
//! let cycles_with_target: Vec<&WorkspaceCycle> = detector
//!     .cycles()
//!     .iter()
//!     .filter(|cycle| {
//!         cycle
//!             .workspace_names()
//!             .contains(&target_workspace.to_string())
//!     })
//!     .collect();
//!
//! println!(
//!     "Found {} cycles involving {}",
//!     cycles_with_target.len(),
//!     target_workspace
//! );
//!
//! for (i, cycle) in cycles_with_target.iter().enumerate() {
//!     println!("\nCycle #{}", i + 1);
//!     println!("Workspaces: {}", cycle.workspace_names().join(" → "));
//!
//!     // Show specific crate-level dependencies
//!     for edge in cycle.edges() {
//!         println!(
//!             "  {} → {} ({:?} dependency)",
//!             edge.from_crate(),
//!             edge.to_crate(),
//!             edge.dependency_type()
//!         );
//!     }
//! }
//! # Ok(())
//! # }
//! ```

// Private modules
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
pub mod common;
pub mod config;
pub mod core;
pub mod detector;
pub mod error;
pub mod executors;
pub mod graph;
pub mod reports;

// Main entry point for the library
pub fn run() -> miette::Result<()> {
    use clap::Parser;

    use crate::cli::{CargoArgs, CargoCommand};
    use crate::commands::execute_command;

    let cargo_args = CargoArgs::parse();
    let CargoCommand::FerrisWheel(cli) = cargo_args.command;

    execute_command(cli.command)
}
