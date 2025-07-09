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
//! use ferris_wheel::analyzer::{CrateMember, Dependency, WorkspaceAnalyzer, WorkspaceInfo};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an analyzer
//! let mut analyzer = WorkspaceAnalyzer::new();
//!
//! // In real usage, you would discover workspaces:
//! // analyzer.discover_workspaces(&[std::env::current_dir()?], None)?;
//!
//! // For this example, let's create a test workspace structure
//! let test_workspace = WorkspaceInfo {
//!     name: "test-workspace".to_string(),
//!     members: vec![CrateMember {
//!         name: "test-crate".to_string(),
//!         dependencies: vec![Dependency {
//!             name: "serde".to_string(),
//!             target: None,
//!         }],
//!         dev_dependencies: vec![],
//!         build_dependencies: vec![],
//!         target_dependencies: HashMap::new(),
//!     }],
//!     is_standalone: false,
//! };
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
