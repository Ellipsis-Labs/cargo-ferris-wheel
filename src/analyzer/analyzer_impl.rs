use std::collections::HashMap;
use std::path::{Path, PathBuf};

use console::style;
use miette::{Diagnostic, Result, WrapErr};
use rayon::prelude::*;
use thiserror::Error;

use super::DependencyClassifier;
use crate::progress::ProgressReporter;
use crate::toml_parser::CargoToml;
use crate::workspace_discovery::{WorkspaceDiscovery, WorkspaceRoot};

#[derive(Error, Debug, Diagnostic)]
pub enum CrateMemberBuilderError {
    #[error("CrateMember name is required")]
    #[diagnostic(
        code(ferris_wheel::analyzer::missing_crate_name),
        help("Provide a name for the crate member using with_name()")
    )]
    MissingName,

    #[error("CrateMember path is required")]
    #[diagnostic(
        code(ferris_wheel::analyzer::missing_crate_path),
        help("Provide a path for the crate member using with_path()")
    )]
    MissingPath,
}

// Type aliases to reduce complexity
type WorkspaceProcessResult = (PathBuf, WorkspaceInfo, Vec<(String, PathBuf)>);
type ParallelProcessResults = Vec<WorkspaceProcessResult>;

#[derive(Debug, Clone)]
pub struct WorkspaceAnalyzer {
    workspaces: HashMap<PathBuf, WorkspaceInfo>,
    crate_to_workspace: HashMap<String, PathBuf>,
    crate_to_paths: HashMap<String, Vec<PathBuf>>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    name: String,
    members: Vec<CrateMember>,
    is_standalone: bool,
}

impl WorkspaceInfo {
    pub fn builder() -> WorkspaceInfoBuilder {
        WorkspaceInfoBuilder::new()
    }

    pub fn members(&self) -> &[CrateMember] {
        &self.members
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_standalone(&self) -> bool {
        self.is_standalone
    }
}

#[derive(Error, Debug, Diagnostic)]
pub enum WorkspaceInfoBuilderError {
    #[error("Workspace name is required")]
    #[diagnostic(
        code(ferris_wheel::analyzer::missing_workspace_name),
        help("Provide a name for the workspace using with_name()")
    )]
    MissingName,

    #[error("Workspace members are required")]
    #[diagnostic(
        code(ferris_wheel::analyzer::missing_workspace_members),
        help("Provide workspace members using with_members()")
    )]
    MissingMembers,
}

#[derive(Default)]
pub struct WorkspaceInfoBuilder {
    name: Option<String>,
    members: Option<Vec<CrateMember>>,
    is_standalone: Option<bool>,
}

impl WorkspaceInfoBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            members: None,
            is_standalone: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_members(mut self, members: Vec<CrateMember>) -> Self {
        self.members = Some(members);
        self
    }

    pub fn with_is_standalone(mut self, is_standalone: bool) -> Self {
        self.is_standalone = Some(is_standalone);
        self
    }

    pub fn build(self) -> Result<WorkspaceInfo, WorkspaceInfoBuilderError> {
        Ok(WorkspaceInfo {
            name: self.name.ok_or(WorkspaceInfoBuilderError::MissingName)?,
            members: self
                .members
                .ok_or(WorkspaceInfoBuilderError::MissingMembers)?,
            is_standalone: self.is_standalone.unwrap_or(false),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CrateMember {
    name: String,
    path: PathBuf,
    dependencies: Vec<Dependency>,
    dev_dependencies: Vec<Dependency>,
    build_dependencies: Vec<Dependency>,
    target_dependencies: HashMap<String, Vec<Dependency>>,
}

impl CrateMember {
    /// Create a new builder for CrateMember
    pub fn builder() -> CrateMemberBuilder {
        CrateMemberBuilder::default()
    }

    pub fn target_dependencies(&self) -> &HashMap<String, Vec<Dependency>> {
        &self.target_dependencies
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn build_dependencies(&self) -> &[Dependency] {
        &self.build_dependencies
    }

    pub fn dev_dependencies(&self) -> &[Dependency] {
        &self.dev_dependencies
    }

    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[derive(Default)]
pub struct CrateMemberBuilder {
    name: Option<String>,
    path: Option<PathBuf>,
    dependencies: Vec<Dependency>,
    dev_dependencies: Vec<Dependency>,
    build_dependencies: Vec<Dependency>,
    target_dependencies: HashMap<String, Vec<Dependency>>,
}

impl CrateMemberBuilder {
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<Dependency>) -> Self {
        self.dependencies = deps;
        self
    }

    pub fn with_dev_dependencies(mut self, deps: Vec<Dependency>) -> Self {
        self.dev_dependencies = deps;
        self
    }

    pub fn with_build_dependencies(mut self, deps: Vec<Dependency>) -> Self {
        self.build_dependencies = deps;
        self
    }

    pub fn with_target_dependencies(mut self, deps: HashMap<String, Vec<Dependency>>) -> Self {
        self.target_dependencies = deps;
        self
    }

    pub fn add_dependency(mut self, dep: Dependency) -> Self {
        self.dependencies.push(dep);
        self
    }

    pub fn build(self) -> Result<CrateMember, CrateMemberBuilderError> {
        Ok(CrateMember {
            name: self.name.ok_or(CrateMemberBuilderError::MissingName)?,
            path: self.path.ok_or(CrateMemberBuilderError::MissingPath)?,
            dependencies: self.dependencies,
            dev_dependencies: self.dev_dependencies,
            build_dependencies: self.build_dependencies,
            target_dependencies: self.target_dependencies,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Dependency {
    name: String,
    target: Option<String>,
}

impl Dependency {
    pub fn builder() -> DependencyBuilder {
        DependencyBuilder::default()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }
}

#[derive(Default)]
pub struct DependencyBuilder {
    name: Option<String>,
    target: Option<String>,
}

#[derive(Error, Debug, Diagnostic)]
pub enum DependencyBuilderError {
    #[error("Dependency name is required")]
    #[diagnostic(
        code(ferris_wheel::analyzer::missing_dependency_name),
        help("Provide a name for the dependency using with_name()")
    )]
    MissingName,
}

impl From<&Dependency> for DependencyBuilder {
    fn from(dep: &Dependency) -> Self {
        Self {
            name: Some(dep.name().to_string()),
            target: dep.target().map(|t| t.to_string()),
        }
    }
}

impl DependencyBuilder {
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    pub fn build(self) -> Result<Dependency, DependencyBuilderError> {
        Ok(Dependency {
            name: self.name.ok_or(DependencyBuilderError::MissingName)?,
            target: self.target,
        })
    }
}

impl Default for WorkspaceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceAnalyzer {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            crate_to_workspace: HashMap::new(),
            crate_to_paths: HashMap::new(),
        }
    }

    pub fn workspaces(&self) -> &HashMap<PathBuf, WorkspaceInfo> {
        &self.workspaces
    }

    pub fn crate_to_workspace(&self) -> &HashMap<String, PathBuf> {
        &self.crate_to_workspace
    }

    pub fn crate_to_paths(&self) -> &HashMap<String, Vec<PathBuf>> {
        &self.crate_to_paths
    }

    pub fn discover_workspaces(
        &mut self,
        paths: &[PathBuf],
        mut progress: Option<&mut ProgressReporter>,
    ) -> Result<()> {
        if let Some(p) = progress.as_mut() {
            p.start_discovery();
        }

        // Discover workspace roots
        let workspace_roots = self.discover_workspace_roots(paths, progress.as_deref())?;

        // Process workspaces and collect errors
        let (results, errors) = self.process_workspaces_parallel(workspace_roots);

        // Report any errors that occurred during processing
        self.report_processing_errors(&errors);

        // Merge successful results
        self.merge_results(results);

        if let Some(p) = progress.as_mut() {
            p.finish_discovery(self.workspaces.len());
        }

        // Report discovery statistics
        self.report_discovery_stats();

        Ok(())
    }

    fn discover_workspace_roots(
        &self,
        paths: &[PathBuf],
        progress: Option<&ProgressReporter>,
    ) -> Result<Vec<WorkspaceRoot>> {
        let mut discovery = WorkspaceDiscovery::new();
        let roots = discovery
            .discover_all(paths, progress)
            .wrap_err("Failed to discover workspaces")?;

        // Report any warnings from discovery
        for warning in discovery.warnings() {
            eprintln!("{} {}", style("⚠").yellow(), warning);
        }

        Ok(roots)
    }

    fn process_workspaces_parallel(
        &self,
        workspace_roots: Vec<WorkspaceRoot>,
    ) -> (ParallelProcessResults, Vec<(String, miette::Error)>) {
        let (successes, errors): (Vec<_>, Vec<_>) = workspace_roots
            .into_par_iter()
            .map(|root| {
                let name = root.name().to_string();
                match self.process_workspace_root_parallel(root) {
                    Ok(result) => Ok(result),
                    Err(e) => Err((name, e)),
                }
            })
            .partition_map(|result| match result {
                Ok(v) => rayon::iter::Either::Left(v),
                Err(e) => rayon::iter::Either::Right(e),
            });

        (successes, errors)
    }

    fn report_processing_errors(&self, errors: &[(String, miette::Error)]) {
        for (workspace_name, error) in errors {
            eprintln!(
                "{} Failed to process workspace '{}': {}",
                style("⚠").yellow(),
                workspace_name,
                error
            );
        }
    }

    fn merge_results(&mut self, results: ParallelProcessResults) {
        for (path, info, crate_mappings) in results {
            // Populate crate_to_paths mapping from the workspace info
            for member in &info.members {
                self.crate_to_paths
                    .entry(member.name.clone())
                    .or_default()
                    .push(member.path.clone());
            }

            self.workspaces.insert(path, info);
            for (crate_name, workspace_path) in crate_mappings {
                self.crate_to_workspace.insert(crate_name, workspace_path);
            }
        }
    }

    fn report_discovery_stats(&self) {
        if self.workspaces.is_empty() {
            eprintln!(
                "{} No Rust workspaces or crates found in the specified paths",
                style("⚠").yellow()
            );
        } else {
            let (workspace_count, standalone_count) = self.count_workspace_types();
            eprintln!(
                "{} Found {} workspace{} and {} standalone crate{}",
                style("✓").green(),
                style(workspace_count).bold(),
                if workspace_count == 1 { "" } else { "s" },
                style(standalone_count).bold(),
                if standalone_count == 1 { "" } else { "s" }
            );
        }
    }

    fn count_workspace_types(&self) -> (usize, usize) {
        let standalone_count = self
            .workspaces
            .values()
            .filter(|ws| ws.is_standalone)
            .count();
        let workspace_count = self.workspaces.len() - standalone_count;
        (workspace_count, standalone_count)
    }

    fn process_workspace_root_parallel(
        &self,
        root: WorkspaceRoot,
    ) -> Result<WorkspaceProcessResult> {
        // Process members in parallel and collect both results and errors
        let results: Vec<Result<(CrateMember, String)>> = root
            .members()
            .par_iter()
            .map(|member| {
                self.analyze_crate_member(
                    member.name(),
                    member.path(),
                    member.cargo_toml(),
                    root.workspace_dependencies(),
                    root.path(),
                )
                .map(|crate_member| (crate_member, member.name().to_string()))
                .wrap_err_with(|| format!("Failed to analyze crate '{}'", member.name()))
            })
            .collect();

        // Separate successful results from errors
        let mut members_with_mappings = Vec::new();
        let mut crate_errors = Vec::new();

        for result in results {
            match result {
                Ok(data) => members_with_mappings.push(data),
                Err(e) => crate_errors.push(e),
            }
        }

        // Report crate-level errors
        for error in &crate_errors {
            eprintln!("{} {}", style("⚠").yellow(), error);
        }

        let members: Vec<CrateMember> = members_with_mappings
            .iter()
            .map(|(m, _)| m.clone())
            .collect();

        let crate_mappings: Vec<(String, PathBuf)> = members_with_mappings
            .into_iter()
            .map(|(_, name)| (name, root.path().clone()))
            .collect();

        let workspace_info = WorkspaceInfo {
            name: root.name().to_string(),
            members,
            is_standalone: root.is_standalone(),
        };

        Ok((root.path().clone(), workspace_info, crate_mappings))
    }

    fn analyze_crate_member(
        &self,
        crate_name: &str,
        crate_path: &Path,
        cargo_toml: &CargoToml,
        workspace_deps: &HashMap<String, PathBuf>,
        _workspace_root: &Path,
    ) -> Result<CrateMember> {
        // Use the new DependencyClassifier to simplify dependency classification
        let classifier = DependencyClassifier::classify_from_toml(cargo_toml, workspace_deps);

        Ok(CrateMember {
            name: crate_name.to_string(),
            path: crate_path.to_path_buf(),
            dependencies: classifier.dependencies().to_vec(),
            dev_dependencies: classifier.dev_dependencies().to_vec(),
            build_dependencies: classifier.build_dependencies().to_vec(),
            target_dependencies: classifier.target_dependencies().clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn create_test_workspace() -> TempDir {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Create workspace
        fs::create_dir_all(root.join("my-workspace")).unwrap();
        fs::write(
            root.join("my-workspace/Cargo.toml"),
            r#"
[workspace]
members = ["crate-a", "crate-b"]

[workspace.dependencies]
atlas-sdk = { path = "../sdk" }
"#,
        )
        .unwrap();
        fs::write(root.join("my-workspace/Cargo.lock"), "# lock").unwrap();

        // Create crate-a
        fs::create_dir_all(root.join("my-workspace/crate-a")).unwrap();
        fs::write(
            root.join("my-workspace/crate-a/Cargo.toml"),
            r#"
[package]
name = "crate-a"

[dependencies]
crate-b = { path = "../crate-b" }
atlas-sdk = { workspace = true }
serde = "1.0"
"#,
        )
        .unwrap();

        // Create crate-b
        fs::create_dir_all(root.join("my-workspace/crate-b")).unwrap();
        fs::write(
            root.join("my-workspace/crate-b/Cargo.toml"),
            r#"
[package]
name = "crate-b"

[dev-dependencies]
crate-a = { path = "../crate-a" }
"#,
        )
        .unwrap();

        temp
    }

    #[test]
    fn test_discover_and_analyze() {
        let temp = create_test_workspace();
        let mut analyzer = WorkspaceAnalyzer::new();

        analyzer
            .discover_workspaces(&[temp.path().to_path_buf()], None)
            .unwrap();

        assert_eq!(analyzer.workspaces().len(), 1);

        let ws = analyzer.workspaces().values().next().unwrap();
        assert_eq!(ws.name, "my-workspace");
        assert_eq!(ws.members.len(), 2);

        // Check crate-a dependencies
        let crate_a = ws.members.iter().find(|m| m.name == "crate-a").unwrap();
        assert_eq!(crate_a.dependencies.len(), 2); // crate-b and atlas-sdk

        // Check crate-b dependencies
        let crate_b = ws.members.iter().find(|m| m.name == "crate-b").unwrap();
        assert_eq!(crate_b.dev_dependencies.len(), 1); // crate-a
    }
}
