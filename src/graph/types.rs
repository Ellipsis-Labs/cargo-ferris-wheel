//! Core graph types
//!
//! This module contains the fundamental data structures used in the dependency
//! graph.

use crate::impl_builder;

/// Represents a workspace node in the dependency graph
#[derive(Debug, Clone)]
pub struct WorkspaceNode {
    pub name: String,
    pub crates: Vec<String>,
}

impl WorkspaceNode {
    pub fn builder() -> WorkspaceNodeBuilder {
        WorkspaceNodeBuilder::new()
    }
}

impl_builder! {
    WorkspaceNodeBuilder => WorkspaceNode {
        name: String,
        crates: Vec<String>,
    }
}

/// Represents a dependency edge between crates
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    pub from_crate: String,
    pub to_crate: String,
    pub dependency_type: DependencyType,
    pub target: Option<String>,
}

impl DependencyEdge {
    pub fn builder() -> DependencyEdgeBuilder {
        DependencyEdgeBuilder::new()
    }
}

pub struct DependencyEdgeBuilder {
    from_crate: Option<String>,
    to_crate: Option<String>,
    dependency_type: Option<DependencyType>,
    target: Option<String>,
}

impl Default for DependencyEdgeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyEdgeBuilder {
    pub fn new() -> Self {
        Self {
            from_crate: None,
            to_crate: None,
            dependency_type: None,
            target: None,
        }
    }

    pub fn with_from_crate(mut self, from_crate: &str) -> Self {
        self.from_crate = Some(from_crate.to_string());
        self
    }

    pub fn with_to_crate(mut self, to_crate: &str) -> Self {
        self.to_crate = Some(to_crate.to_string());
        self
    }

    pub fn with_dependency_type(mut self, dependency_type: DependencyType) -> Self {
        self.dependency_type = Some(dependency_type);
        self
    }

    pub fn with_target(mut self, target: Option<String>) -> Self {
        self.target = target;
        self
    }
}

impl crate::common::ConfigBuilder for DependencyEdgeBuilder {
    type Config = DependencyEdge;

    fn build(self) -> Result<Self::Config, crate::error::FerrisWheelError> {
        Ok(DependencyEdge {
            from_crate: self.from_crate.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: from_crate".to_string(),
                }
            })?,
            to_crate: self.to_crate.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: to_crate".to_string(),
                }
            })?,
            dependency_type: self.dependency_type.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: dependency_type".to_string(),
                }
            })?,
            target: self.target,
        })
    }
}

/// Type of dependency relationship
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DependencyType {
    Normal,
    Dev,
    Build,
}
