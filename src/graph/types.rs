//! Core graph types
//!
//! This module contains the fundamental data structures used in the dependency
//! graph.

/// Represents a workspace node in the dependency graph
#[derive(Debug, Clone)]
pub struct WorkspaceNode {
    name: String,
    crates: Vec<String>,
}

impl WorkspaceNode {
    pub fn builder() -> WorkspaceNodeBuilder {
        WorkspaceNodeBuilder::new()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn crates(&self) -> &[String] {
        &self.crates
    }
}

#[derive(Default)]
pub struct WorkspaceNodeBuilder {
    name: Option<String>,
    crates: Option<Vec<String>>,
}

impl WorkspaceNodeBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            crates: None,
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_crates(mut self, crates: Vec<String>) -> Self {
        self.crates = Some(crates);
        self
    }
}

impl crate::common::ConfigBuilder for WorkspaceNodeBuilder {
    type Config = WorkspaceNode;

    fn build(self) -> Result<Self::Config, crate::error::FerrisWheelError> {
        Ok(WorkspaceNode {
            name: self
                .name
                .ok_or_else(|| crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: name".to_string(),
                })?,
            crates: self.crates.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: crates".to_string(),
                }
            })?,
        })
    }
}

/// Represents a dependency edge between crates
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    from_crate: String,
    to_crate: String,
    dependency_type: DependencyType,
    target: Option<String>,
}

impl DependencyEdge {
    pub fn builder() -> DependencyEdgeBuilder {
        DependencyEdgeBuilder::new()
    }

    pub fn from_crate(&self) -> &str {
        &self.from_crate
    }

    pub fn to_crate(&self) -> &str {
        &self.to_crate
    }

    pub fn dependency_type(&self) -> &DependencyType {
        &self.dependency_type
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
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
