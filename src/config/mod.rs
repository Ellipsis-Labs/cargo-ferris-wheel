//! # Configuration Module
//!
//! This module provides configuration structures for all ferris-wheel commands.
//! Each command has its own config module with builder patterns for easy
//! construction.
//!
//! ## Command Configurations
//!
//! - **CheckCyclesConfig**: Configuration for the `check` command to detect
//!   cycles
//! - **AnalyzeCrateConfig**: Configuration for the `analyze` command to examine
//!   specific crates
//! - **WorkspaceDepsConfig**: Configuration for the `deps` command for CI
//!   optimization
//! - **GraphOptions**: Configuration for the `graph` command to visualize
//!   dependencies
//!
//! ## Example
//!
//! ```
//! use ferris_wheel::cli::{GraphFormat, OutputFormat};
//! use ferris_wheel::config::{CheckCyclesConfig, GraphOptions};
//!
//! // Each configuration struct provides a builder pattern
//! // The builders are generated with the impl_builder! macro
//! // and provide with_* methods for each field
//!
//! // Example: Create a CheckCyclesConfig
//! let builder = CheckCyclesConfig::builder()
//!     .with_paths(vec![".".into()])
//!     .with_format(OutputFormat::Human)
//!     .with_error_on_cycles(true);
//!
//! // Example: Create a GraphOptions config
//! let graph_builder = GraphOptions::builder()
//!     .with_paths(vec![".".into()])
//!     .with_format(GraphFormat::Dot)
//!     .with_highlight_cycles(true);
//! ```

pub mod affected;
pub mod analyze;
pub mod check;
pub mod deps;
pub mod graph;

pub use affected::AffectedConfig;
pub use analyze::AnalyzeCrateConfig;
pub use check::CheckCyclesConfig;
pub use deps::WorkspaceDepsConfig;
pub use graph::GraphOptions;
