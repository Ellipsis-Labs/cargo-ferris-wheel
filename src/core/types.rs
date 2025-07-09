//! Core type definitions
//!
//! This module contains the basic data structures used throughout the
//! application, with minimal logic - focusing on data representation.

use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a Rust workspace
#[derive(Debug, Clone)]
pub struct Workspace {
    path: PathBuf,
    name: String,
    members: Vec<Crate>,
}

impl Workspace {
    /// Creates a new workspace builder
    pub fn builder() -> WorkspaceBuilder {
        WorkspaceBuilder::default()
    }

    /// Gets the workspace path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Gets the workspace name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gets the workspace members
    pub fn members(&self) -> &[Crate] {
        &self.members
    }

    /// Gets the workspace members mutably
    pub fn members_mut(&mut self) -> &mut Vec<Crate> {
        &mut self.members
    }
}

/// Builder for Workspace
#[derive(Default)]
pub struct WorkspaceBuilder {
    path: Option<PathBuf>,
    name: Option<String>,
    members: Vec<Crate>,
}

impl WorkspaceBuilder {
    /// Sets the workspace path
    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Sets the workspace name
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the workspace members
    pub fn members(mut self, members: Vec<Crate>) -> Self {
        self.members = members;
        self
    }

    /// Adds a member to the workspace
    pub fn add_member(mut self, member: Crate) -> Self {
        self.members.push(member);
        self
    }

    /// Builds the Workspace
    pub fn build(self) -> Result<Workspace, &'static str> {
        let path = self.path.ok_or("path is required")?;
        let name = self.name.ok_or("name is required")?;

        Ok(Workspace {
            path,
            name,
            members: self.members,
        })
    }
}

/// Represents a Rust crate within a workspace
#[derive(Debug, Clone)]
pub struct Crate {
    name: String,
    path: PathBuf,
    dependencies: Dependencies,
}

impl Crate {
    /// Creates a new crate builder
    pub fn builder() -> CrateBuilder {
        CrateBuilder::default()
    }

    /// Gets the crate name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gets the crate path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Gets the crate dependencies
    pub fn dependencies(&self) -> &Dependencies {
        &self.dependencies
    }

    /// Gets the crate dependencies mutably
    pub fn dependencies_mut(&mut self) -> &mut Dependencies {
        &mut self.dependencies
    }
}

/// Builder for Crate
#[derive(Default)]
pub struct CrateBuilder {
    name: Option<String>,
    path: Option<PathBuf>,
    dependencies: Option<Dependencies>,
}

impl CrateBuilder {
    /// Sets the crate name
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the crate path
    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Sets the crate dependencies
    pub fn dependencies(mut self, dependencies: Dependencies) -> Self {
        self.dependencies = Some(dependencies);
        self
    }

    /// Builds the Crate
    pub fn build(self) -> Result<Crate, &'static str> {
        let name = self.name.ok_or("name is required")?;
        let path = self.path.ok_or("path is required")?;
        let dependencies = self.dependencies.unwrap_or_default();

        Ok(Crate {
            name,
            path,
            dependencies,
        })
    }
}

/// Dependencies organized by type
#[derive(Debug, Clone, Default)]
pub struct Dependencies {
    normal: Vec<DependencyRef>,
    dev: Vec<DependencyRef>,
    build: Vec<DependencyRef>,
    target: HashMap<String, Vec<DependencyRef>>,
}

impl Dependencies {
    /// Creates a new dependencies builder
    pub fn builder() -> DependenciesBuilder {
        DependenciesBuilder::default()
    }

    /// Gets normal dependencies
    pub fn normal(&self) -> &[DependencyRef] {
        &self.normal
    }

    /// Gets dev dependencies
    pub fn dev(&self) -> &[DependencyRef] {
        &self.dev
    }

    /// Gets build dependencies
    pub fn build(&self) -> &[DependencyRef] {
        &self.build
    }

    /// Gets target-specific dependencies
    pub fn target(&self) -> &HashMap<String, Vec<DependencyRef>> {
        &self.target
    }

    /// Gets normal dependencies mutably
    pub fn normal_mut(&mut self) -> &mut Vec<DependencyRef> {
        &mut self.normal
    }

    /// Gets dev dependencies mutably
    pub fn dev_mut(&mut self) -> &mut Vec<DependencyRef> {
        &mut self.dev
    }

    /// Gets build dependencies mutably
    pub fn build_mut(&mut self) -> &mut Vec<DependencyRef> {
        &mut self.build
    }

    /// Gets target-specific dependencies mutably
    pub fn target_mut(&mut self) -> &mut HashMap<String, Vec<DependencyRef>> {
        &mut self.target
    }
}

/// Builder for Dependencies
#[derive(Default)]
pub struct DependenciesBuilder {
    normal: Vec<DependencyRef>,
    dev: Vec<DependencyRef>,
    build: Vec<DependencyRef>,
    target: HashMap<String, Vec<DependencyRef>>,
}

impl DependenciesBuilder {
    /// Sets normal dependencies
    pub fn normal(mut self, deps: Vec<DependencyRef>) -> Self {
        self.normal = deps;
        self
    }

    /// Adds a normal dependency
    pub fn add_normal(mut self, dep: DependencyRef) -> Self {
        self.normal.push(dep);
        self
    }

    /// Sets dev dependencies
    pub fn dev(mut self, deps: Vec<DependencyRef>) -> Self {
        self.dev = deps;
        self
    }

    /// Adds a dev dependency
    pub fn add_dev(mut self, dep: DependencyRef) -> Self {
        self.dev.push(dep);
        self
    }

    /// Sets build dependencies
    pub fn build_deps(mut self, deps: Vec<DependencyRef>) -> Self {
        self.build = deps;
        self
    }

    /// Adds a build dependency
    pub fn add_build(mut self, dep: DependencyRef) -> Self {
        self.build.push(dep);
        self
    }

    /// Sets target-specific dependencies
    pub fn target(mut self, target: HashMap<String, Vec<DependencyRef>>) -> Self {
        self.target = target;
        self
    }

    /// Adds target-specific dependencies
    pub fn add_target(mut self, target_name: String, deps: Vec<DependencyRef>) -> Self {
        self.target.insert(target_name, deps);
        self
    }

    /// Builds the Dependencies
    pub fn build(self) -> Dependencies {
        Dependencies {
            normal: self.normal,
            dev: self.dev,
            build: self.build,
            target: self.target,
        }
    }
}

/// Reference to a dependency
#[derive(Debug, Clone)]
pub struct DependencyRef {
    name: String,
    path: Option<PathBuf>,
    workspace: bool,
    target: Option<String>,
}

impl DependencyRef {
    /// Creates a new dependency reference builder
    pub fn builder() -> DependencyRefBuilder {
        DependencyRefBuilder::default()
    }

    /// Gets the dependency name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gets the dependency path
    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// Checks if this is a workspace dependency
    pub fn is_workspace(&self) -> bool {
        self.workspace
    }

    /// Gets the target platform
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }
}

/// Builder for DependencyRef
#[derive(Default)]
pub struct DependencyRefBuilder {
    name: Option<String>,
    path: Option<PathBuf>,
    workspace: bool,
    target: Option<String>,
}

impl DependencyRefBuilder {
    /// Sets the dependency name
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the dependency path
    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Sets whether this is a workspace dependency
    pub fn workspace(mut self, workspace: bool) -> Self {
        self.workspace = workspace;
        self
    }

    /// Sets the target platform
    pub fn target(mut self, target: String) -> Self {
        self.target = Some(target);
        self
    }

    /// Builds the DependencyRef
    pub fn build(self) -> Result<DependencyRef, &'static str> {
        let name = self.name.ok_or("name is required")?;

        Ok(DependencyRef {
            name,
            path: self.path,
            workspace: self.workspace,
            target: self.target,
        })
    }
}

/// A cycle in the dependency graph
#[derive(Debug, Clone)]
pub struct Cycle {
    participants: Vec<String>,
    edges: Vec<Edge>,
}

impl Cycle {
    /// Creates a new cycle builder
    pub fn builder() -> CycleBuilder {
        CycleBuilder::default()
    }

    /// Gets the cycle participants
    pub fn participants(&self) -> &[String] {
        &self.participants
    }

    /// Gets the cycle edges
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
}

/// Builder for Cycle
#[derive(Default)]
pub struct CycleBuilder {
    participants: Vec<String>,
    edges: Vec<Edge>,
}

impl CycleBuilder {
    /// Sets the cycle participants
    pub fn participants(mut self, participants: Vec<String>) -> Self {
        self.participants = participants;
        self
    }

    /// Adds a participant to the cycle
    pub fn add_participant(mut self, participant: String) -> Self {
        self.participants.push(participant);
        self
    }

    /// Sets the cycle edges
    pub fn edges(mut self, edges: Vec<Edge>) -> Self {
        self.edges = edges;
        self
    }

    /// Adds an edge to the cycle
    pub fn add_edge(mut self, edge: Edge) -> Self {
        self.edges.push(edge);
        self
    }

    /// Builds the Cycle
    pub fn build(self) -> Cycle {
        Cycle {
            participants: self.participants,
            edges: self.edges,
        }
    }
}

/// An edge in the dependency graph
#[derive(Debug, Clone)]
pub struct Edge {
    from: String,
    to: String,
    dependency_type: EdgeType,
    target: Option<String>,
}

impl Edge {
    /// Creates a new edge builder
    pub fn builder() -> EdgeBuilder {
        EdgeBuilder::default()
    }

    /// Gets the source node
    pub fn from(&self) -> &str {
        &self.from
    }

    /// Gets the destination node
    pub fn to(&self) -> &str {
        &self.to
    }

    /// Gets the dependency type
    pub fn dependency_type(&self) -> EdgeType {
        self.dependency_type
    }

    /// Gets the target platform
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }
}

/// Builder for Edge
#[derive(Default)]
pub struct EdgeBuilder {
    from: Option<String>,
    to: Option<String>,
    dependency_type: Option<EdgeType>,
    target: Option<String>,
}

impl EdgeBuilder {
    /// Sets the source node
    pub fn from(mut self, from: String) -> Self {
        self.from = Some(from);
        self
    }

    /// Sets the destination node
    pub fn to(mut self, to: String) -> Self {
        self.to = Some(to);
        self
    }

    /// Sets the dependency type
    pub fn dependency_type(mut self, dependency_type: EdgeType) -> Self {
        self.dependency_type = Some(dependency_type);
        self
    }

    /// Sets the target platform
    pub fn target(mut self, target: String) -> Self {
        self.target = Some(target);
        self
    }

    /// Builds the Edge
    pub fn build(self) -> Result<Edge, &'static str> {
        let from = self.from.ok_or("from is required")?;
        let to = self.to.ok_or("to is required")?;
        let dependency_type = self.dependency_type.ok_or("dependency_type is required")?;

        Ok(Edge {
            from,
            to,
            dependency_type,
            target: self.target,
        })
    }
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
