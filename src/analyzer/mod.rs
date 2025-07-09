//! # Workspace Analysis Module
//!
//! This module is responsible for discovering and analyzing Rust workspaces
//! in a monorepo structure. It identifies workspace boundaries, member crates,
//! and their dependency relationships.
//!
//! ## Key Components
//!
//! - **WorkspaceAnalyzer**: Main analyzer that discovers workspaces and their
//!   crates
//! - **DependencyClassifier**: Classifies dependencies by type (normal, dev,
//!   build, target)
//! - **WorkspaceInfo**: Contains metadata about a discovered workspace
//! - **CrateMember**: Represents a crate within a workspace
//!
//! ## Example
//!
//! ```
//! use std::collections::HashMap;
//! use std::path::PathBuf;
//!
//! use cargo_ferris_wheel::analyzer::{CrateMember, Dependency, WorkspaceAnalyzer, WorkspaceInfo};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an analyzer
//! let mut analyzer = WorkspaceAnalyzer::new();
//!
//! // In real usage, you would discover workspaces:
//! // analyzer.discover_workspaces(&[std::env::current_dir()?], None)?;
//!
//! // For this example, let's create a test workspace structure
//! let test_workspace = WorkspaceInfo::builder()
//!     .with_name("test-workspace")
//!     .with_members(vec![
//!         CrateMember::builder()
//!             .with_name("test-crate")
//!             .with_path(PathBuf::from("test-crate"))
//!             .with_dependencies(vec![
//!                 Dependency::builder().with_name("serde").build().unwrap(),
//!             ])
//!             .build()
//!             .unwrap(),
//!     ])
//!     .build()
//!     .unwrap();
//!
//! // The analyzer stores workspaces internally
//! // In real usage, discover_workspaces() populates these
//! assert!(analyzer.workspaces().is_empty());
//! assert!(analyzer.crate_to_workspace().is_empty());
//! # Ok(())
//! # }
//! ```

mod dependency_classifier;

pub use dependency_classifier::DependencyClassifier;

// Re-export the main analyzer types
mod analyzer_impl;
pub use analyzer_impl::*;
