//! Core type definitions
//!
//! This module contains the basic data structures used throughout the
//! application, with minimal logic - focusing on data representation.

use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a Rust workspace
#[derive(Debug, Clone)]
pub struct Workspace {
    pub path: PathBuf,
    pub name: String,
    pub members: Vec<Crate>,
}

/// Represents a Rust crate within a workspace
#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    pub path: PathBuf,
    pub dependencies: Dependencies,
}

/// Dependencies organized by type
#[derive(Debug, Clone, Default)]
pub struct Dependencies {
    pub normal: Vec<DependencyRef>,
    pub dev: Vec<DependencyRef>,
    pub build: Vec<DependencyRef>,
    pub target: HashMap<String, Vec<DependencyRef>>,
}

/// Reference to a dependency
#[derive(Debug, Clone)]
pub struct DependencyRef {
    pub name: String,
    pub path: Option<PathBuf>,
    pub workspace: bool,
    pub target: Option<String>,
}

/// A cycle in the dependency graph
#[derive(Debug, Clone)]
pub struct Cycle {
    pub participants: Vec<String>,
    pub edges: Vec<Edge>,
}

/// An edge in the dependency graph
#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub dependency_type: EdgeType,
    pub target: Option<String>,
}

/// Type of dependency edge
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeType {
    Normal,
    Dev,
    Build,
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::Normal => write!(f, "normal"),
            EdgeType::Dev => write!(f, "dev"),
            EdgeType::Build => write!(f, "build"),
        }
    }
}
