use std::collections::HashMap;
use std::path::{Path, PathBuf};

use console::style;
use miette::{Result, WrapErr};
use rayon::prelude::*;
use thiserror::Error;

use super::DependencyClassifier;
use crate::progress::ProgressReporter;
use crate::toml_parser::CargoToml;
use crate::workspace_discovery::{WorkspaceDiscovery, WorkspaceRoot};

#[derive(Error, Debug)]
pub enum CrateMemberBuilderError {
    #[error("CrateMember name is required")]
    MissingName,
    #[error("CrateMember path is required")]
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
    pub name: String,
    pub members: Vec<CrateMember>,
    pub is_standalone: bool,
}

#[derive(Debug, Clone)]
pub struct CrateMember {
    pub name: String,
    pub path: PathBuf,
    pub dependencies: Vec<Dependency>,
    pub dev_dependencies: Vec<Dependency>,
    pub build_dependencies: Vec<Dependency>,
    pub target_dependencies: HashMap<String, Vec<Dependency>>,
}

impl CrateMember {
    /// Create a new builder for CrateMember
    pub fn builder() -> CrateMemberBuilder {
        CrateMemberBuilder::default()
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
    pub name: String,
    pub target: Option<String>,
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
        discovery
            .discover_all(paths, progress)
            .wrap_err("Failed to discover workspaces")
    }

    fn process_workspaces_parallel(
        &self,
        workspace_roots: Vec<WorkspaceRoot>,
    ) -> (ParallelProcessResults, Vec<(String, miette::Error)>) {
        let (successes, errors): (Vec<_>, Vec<_>) = workspace_roots
            .into_par_iter()
            .map(|root| {
                let name = root.name.clone();
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
        // Process members in parallel
        let members_with_mappings: Vec<(CrateMember, String)> = root
            .members
            .par_iter()
            .filter_map(|member| {
                match self.analyze_crate_member(
                    &member.name,
                    &member.path,
                    &member.cargo_toml,
                    &root.workspace_dependencies,
                    &root.path,
                ) {
                    Ok(crate_member) => Some((crate_member, member.name.clone())),
                    Err(e) => {
                        eprintln!(
                            "{} Failed to analyze crate {}: {}",
                            style("⚠").yellow(),
                            member.name,
                            e
                        );
                        None
                    }
                }
            })
            .collect();

        let members: Vec<CrateMember> = members_with_mappings
            .iter()
            .map(|(m, _)| m.clone())
            .collect();

        let crate_mappings: Vec<(String, PathBuf)> = members_with_mappings
            .into_iter()
            .map(|(_, name)| (name, root.path.clone()))
            .collect();

        let workspace_info = WorkspaceInfo {
            name: root.name.clone(),
            members,
            is_standalone: root.is_standalone,
        };

        Ok((root.path.clone(), workspace_info, crate_mappings))
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
            dependencies: classifier.dependencies,
            dev_dependencies: classifier.dev_dependencies,
            build_dependencies: classifier.build_dependencies,
            target_dependencies: classifier.target_dependencies,
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
