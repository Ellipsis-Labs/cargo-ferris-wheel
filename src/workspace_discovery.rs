use std::collections::HashSet;
use std::path::{Path, PathBuf};

use miette::{Result, WrapErr};
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::progress::ProgressReporter;
use crate::toml_parser::CargoToml;

pub struct WorkspaceDiscovery {
    discovered_roots: HashSet<PathBuf>,
    /// Warnings collected during discovery that didn't prevent processing
    warnings: Vec<String>,
}

impl WorkspaceDiscovery {
    pub fn new() -> Self {
        Self {
            discovered_roots: HashSet::new(),
            warnings: Vec::new(),
        }
    }

    /// Get warnings collected during discovery
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Discover all workspace roots and standalone crates in the given paths
    ///
    /// Returns discovered workspace roots. Any non-fatal errors (like invalid
    /// Cargo.toml files) are collected as warnings and can be retrieved
    /// with `warnings()`.
    pub fn discover_all(
        &mut self,
        paths: &[PathBuf],
        progress: Option<&ProgressReporter>,
    ) -> Result<Vec<WorkspaceRoot>> {
        let mut roots = Vec::new();

        for path in paths {
            if !path.exists() {
                self.warnings
                    .push(format!("Path '{}' does not exist", path.display()));
                continue;
            }

            if !path.is_dir() {
                self.warnings
                    .push(format!("Path '{}' is not a directory", path.display()));
                continue;
            }

            self.discover_in_path(path, &mut roots, progress)
                .wrap_err_with(|| {
                    format!("Failed to discover workspaces in '{}'", path.display())
                })?;
        }

        // Sort by path for consistent output
        roots.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(roots)
    }

    fn discover_in_path(
        &mut self,
        path: &Path,
        roots: &mut Vec<WorkspaceRoot>,
        progress: Option<&ProgressReporter>,
    ) -> Result<()> {
        // First, look for Cargo.lock files as they indicate workspace roots or
        // standalone crates
        let lock_files: Vec<PathBuf> = WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name();
                // Skip common directories that won't contain Cargo.lock
                name != "target" && name != ".git" && name != "node_modules"
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() == "Cargo.lock")
            .map(|e| e.into_path())
            .collect();

        // Process each Cargo.lock location in parallel
        // First, filter to unique directories
        let unique_dirs: Vec<PathBuf> = lock_files
            .into_iter()
            .filter_map(|lock_path| {
                let dir = lock_path.parent()?.to_path_buf();
                if self.discovered_roots.insert(dir.clone()) {
                    Some(dir)
                } else {
                    None
                }
            })
            .collect();

        // Then process in parallel
        let results: Vec<(Option<WorkspaceRoot>, Vec<String>)> = unique_dirs
            .into_par_iter()
            .map(|dir| {
                let mut local_warnings = Vec::new();
                let cargo_toml_path = dir.join("Cargo.toml");
                if !cargo_toml_path.exists() {
                    return (None, local_warnings);
                }

                if let Some(p) = progress {
                    p.checking_manifest(&cargo_toml_path);
                }

                match CargoToml::parse_file(&cargo_toml_path) {
                    Ok(cargo_toml) => {
                        if cargo_toml.is_workspace_root() {
                            // This is a workspace root
                            // We need to expand members sequentially for now
                            // due to borrow checker constraints
                            (
                                match WorkspaceRoot::builder()
                                    .path(dir.clone())
                                    .name(
                                        dir.file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string(),
                                    )
                                    .members(Vec::new()) // Will be populated later
                                    .workspace_dependencies(cargo_toml.get_workspace_dependencies())
                                    .with_is_standalone(false)
                                    .build()
                                {
                                    Ok(root) => Some(root),
                                    Err(e) => {
                                        local_warnings
                                            .push(format!("Failed to build workspace root: {e}"));
                                        None
                                    }
                                },
                                local_warnings,
                            )
                        } else if let Some(package) = cargo_toml.package.clone() {
                            // This is a standalone crate
                            (
                                match WorkspaceMember::builder()
                                    .path(dir.clone())
                                    .name(package.name.clone())
                                    .cargo_toml(cargo_toml)
                                    .build()
                                {
                                    Ok(member) => {
                                        match WorkspaceRoot::builder()
                                            .path(dir)
                                            .name(package.name.clone())
                                            .members(vec![member])
                                            .workspace_dependencies(Default::default())
                                            .with_is_standalone(true)
                                            .build()
                                        {
                                            Ok(root) => Some(root),
                                            Err(e) => {
                                                local_warnings.push(format!(
                                                    "Failed to build workspace root: {e}",
                                                ));
                                                None
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        local_warnings.push(format!(
                                            "Failed to build workspace member: {e}",
                                        ));
                                        None
                                    }
                                },
                                local_warnings,
                            )
                        } else {
                            (None, local_warnings)
                        }
                    }
                    Err(e) => {
                        local_warnings.push(format!(
                            "Failed to parse {}: {}",
                            cargo_toml_path.display(),
                            e
                        ));
                        (None, local_warnings)
                    }
                }
            })
            .collect();

        // Separate roots and warnings
        let mut new_roots = Vec::new();
        for (root, warnings) in results {
            if let Some(r) = root {
                new_roots.push(r);
            }
            self.warnings.extend(warnings);
        }

        // Expand workspace members for workspace roots
        for mut root in new_roots {
            if !root.is_standalone && root.members.is_empty() {
                let cargo_toml_path = root.path.join("Cargo.toml");
                match CargoToml::parse_file(&cargo_toml_path) {
                    Ok(cargo_toml) => {
                        match self.expand_workspace_members(&root.path, &cargo_toml) {
                            Ok(members) => root.members = members,
                            Err(e) => {
                                self.warnings.push(format!(
                                    "Failed to expand members for workspace '{}': {}",
                                    root.name, e
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        self.warnings.push(format!(
                            "Failed to parse Cargo.toml for workspace '{}': {}",
                            root.name, e
                        ));
                    }
                }
            }
            roots.push(root);
        }

        // Also check for workspace roots without Cargo.lock (less common but possible)
        self.find_additional_workspaces(path, roots, progress)?;

        Ok(())
    }

    fn find_additional_workspaces(
        &mut self,
        path: &Path,
        roots: &mut Vec<WorkspaceRoot>,
        progress: Option<&ProgressReporter>,
    ) -> Result<()> {
        // Look for Cargo.toml files with [workspace] sections
        for entry in WalkDir::new(path)
            .max_depth(3) // Don't go too deep
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name();
                name != "target" && name != ".git" && name != "node_modules"
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() == "Cargo.toml")
        {
            let cargo_toml_path = entry.path();
            let Some(dir) = cargo_toml_path.parent() else {
                continue;
            };

            // Skip if already processed
            if self.discovered_roots.contains(dir) {
                continue;
            }

            if let Some(p) = progress {
                p.checking_manifest(cargo_toml_path);
            }

            match CargoToml::parse_file(cargo_toml_path) {
                Ok(cargo_toml) if cargo_toml.is_workspace_root() => {
                    self.discovered_roots.insert(dir.to_path_buf());
                    match self.expand_workspace_members(dir, &cargo_toml) {
                        Ok(members) => {
                            roots.push(WorkspaceRoot {
                                path: dir.to_path_buf(),
                                name: dir
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string(),
                                members,
                                workspace_dependencies: cargo_toml.get_workspace_dependencies(),
                                is_standalone: false,
                            });
                        }
                        Err(e) => {
                            self.warnings.push(format!(
                                "Failed to expand members for workspace at '{}': {}",
                                dir.display(),
                                e
                            ));
                        }
                    }
                }
                Ok(_) => {} // Not a workspace root
                Err(e) => {
                    self.warnings.push(format!(
                        "Failed to parse {}: {}",
                        cargo_toml_path.display(),
                        e
                    ));
                }
            }
        }

        Ok(())
    }

    fn expand_workspace_members(
        &mut self,
        workspace_root: &Path,
        cargo_toml: &CargoToml,
    ) -> Result<Vec<WorkspaceMember>> {
        let mut members = Vec::new();
        let member_patterns = cargo_toml.get_workspace_members();

        // Parallelize member expansion
        let results: Vec<(Vec<WorkspaceMember>, Vec<String>)> = member_patterns
            .into_par_iter()
            .map(|pattern| {
                let mut local_members = Vec::new();
                let mut local_warnings = Vec::new();

                // Handle glob patterns
                if pattern.contains('*') {
                    let glob_pattern = workspace_root.join(&pattern);
                    let glob_str = glob_pattern.to_string_lossy();

                    match glob::glob(&glob_str) {
                        Ok(paths) => {
                            let member_paths: Vec<PathBuf> =
                                paths.flatten().filter(|path| path.is_dir()).collect();

                            let inner_results: Vec<(Option<WorkspaceMember>, Vec<String>)> =
                                member_paths
                                    .into_par_iter()
                                    .map(|path| match self.load_member_single(&path) {
                                        Ok(Some(member)) => (Some(member), vec![]),
                                        Ok(None) => (None, vec![]),
                                        Err(e) => {
                                            let warning = format!(
                                                "Failed to load member {}: {}",
                                                path.display(),
                                                e
                                            );
                                            (None, vec![warning])
                                        }
                                    })
                                    .collect();

                            for (member, warnings) in inner_results {
                                if let Some(m) = member {
                                    local_members.push(m);
                                }
                                local_warnings.extend(warnings);
                            }
                        }
                        Err(e) => {
                            local_warnings.push(format!("Invalid glob pattern '{pattern}': {e}"));
                        }
                    }
                } else {
                    // Direct path
                    let member_path = workspace_root.join(&pattern);
                    if member_path.is_dir() {
                        match self.load_member_single(&member_path) {
                            Ok(Some(member)) => local_members.push(member),
                            Ok(None) => {}
                            Err(e) => {
                                local_warnings.push(format!(
                                    "Failed to load member {}: {}",
                                    member_path.display(),
                                    e
                                ));
                            }
                        }
                    }
                }

                (local_members, local_warnings)
            })
            .collect();

        // Collect results and warnings
        for (local_members, local_warnings) in results {
            members.extend(local_members);
            self.warnings.extend(local_warnings);
        }

        Ok(members)
    }

    fn load_member_single(&self, path: &Path) -> Result<Option<WorkspaceMember>> {
        let cargo_toml_path = path.join("Cargo.toml");
        if cargo_toml_path.exists() {
            let cargo_toml = CargoToml::parse_file(&cargo_toml_path).wrap_err_with(|| {
                format!(
                    "Failed to parse member Cargo.toml at {}",
                    cargo_toml_path.display()
                )
            })?;

            if let Some(package) = &cargo_toml.package {
                Ok(Some(
                    WorkspaceMember::builder()
                        .path(path.to_path_buf())
                        .name(package.name.clone())
                        .cargo_toml(cargo_toml)
                        .build()
                        .wrap_err_with(|| {
                            format!(
                                "Failed to build workspace member from {}",
                                cargo_toml_path.display()
                            )
                        })?,
                ))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceRoot {
    path: PathBuf,
    name: String,
    members: Vec<WorkspaceMember>,
    workspace_dependencies: std::collections::HashMap<String, PathBuf>,
    is_standalone: bool,
}

impl WorkspaceRoot {
    /// Creates a new workspace root builder
    pub fn builder() -> WorkspaceRootBuilder {
        WorkspaceRootBuilder::default()
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
    pub fn members(&self) -> &[WorkspaceMember] {
        &self.members
    }

    /// Gets the workspace dependencies
    pub fn workspace_dependencies(&self) -> &std::collections::HashMap<String, PathBuf> {
        &self.workspace_dependencies
    }

    /// Checks if this is a standalone crate
    pub fn is_standalone(&self) -> bool {
        self.is_standalone
    }
}

/// Builder for WorkspaceRoot
#[derive(Default)]
pub struct WorkspaceRootBuilder {
    path: Option<PathBuf>,
    name: Option<String>,
    members: Vec<WorkspaceMember>,
    workspace_dependencies: std::collections::HashMap<String, PathBuf>,
    is_standalone: bool,
}

impl WorkspaceRootBuilder {
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
    pub fn members(mut self, members: Vec<WorkspaceMember>) -> Self {
        self.members = members;
        self
    }

    /// Sets the workspace dependencies
    pub fn workspace_dependencies(
        mut self,
        deps: std::collections::HashMap<String, PathBuf>,
    ) -> Self {
        self.workspace_dependencies = deps;
        self
    }

    /// Sets whether this is a standalone crate
    pub fn with_is_standalone(mut self, is_standalone: bool) -> Self {
        self.is_standalone = is_standalone;
        self
    }

    /// Builds the WorkspaceRoot
    pub fn build(self) -> Result<WorkspaceRoot, &'static str> {
        let path = self.path.ok_or("path is required")?;
        let name = self.name.ok_or("name is required")?;

        Ok(WorkspaceRoot {
            path,
            name,
            members: self.members,
            workspace_dependencies: self.workspace_dependencies,
            is_standalone: self.is_standalone,
        })
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceMember {
    path: PathBuf,
    name: String,
    cargo_toml: CargoToml,
}

impl WorkspaceMember {
    /// Creates a new workspace member builder
    pub fn builder() -> WorkspaceMemberBuilder {
        WorkspaceMemberBuilder::default()
    }

    /// Gets the member path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Gets the member name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gets the cargo.toml contents
    pub fn cargo_toml(&self) -> &CargoToml {
        &self.cargo_toml
    }
}

/// Builder for WorkspaceMember
#[derive(Default)]
pub struct WorkspaceMemberBuilder {
    path: Option<PathBuf>,
    name: Option<String>,
    cargo_toml: Option<CargoToml>,
}

impl WorkspaceMemberBuilder {
    /// Sets the member path
    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Sets the member name
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the cargo.toml contents
    pub fn cargo_toml(mut self, cargo_toml: CargoToml) -> Self {
        self.cargo_toml = Some(cargo_toml);
        self
    }

    /// Builds the WorkspaceMember
    pub fn build(self) -> Result<WorkspaceMember, crate::error::FerrisWheelError> {
        let path = self
            .path
            .ok_or(crate::error::FerrisWheelError::ConfigurationError {
                message: "path is required".to_string(),
            })?;
        let name = self
            .name
            .ok_or(crate::error::FerrisWheelError::ConfigurationError {
                message: "name is required".to_string(),
            })?;
        let cargo_toml =
            self.cargo_toml
                .ok_or(crate::error::FerrisWheelError::ConfigurationError {
                    message: "cargo_toml is required".to_string(),
                })?;

        Ok(WorkspaceMember {
            path,
            name,
            cargo_toml,
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

        // Create workspace root
        fs::create_dir_all(root.join("workspace")).unwrap();
        fs::write(
            root.join("workspace/Cargo.toml"),
            r#"
[workspace]
members = ["crate-a", "crate-b"]

[workspace.dependencies]
shared = { path = "../shared" }
"#,
        )
        .unwrap();
        fs::write(root.join("workspace/Cargo.lock"), "# lock file").unwrap();

        // Create member crates
        fs::create_dir_all(root.join("workspace/crate-a")).unwrap();
        fs::write(
            root.join("workspace/crate-a/Cargo.toml"),
            r#"
[package]
name = "crate-a"

[dependencies]
crate-b = { path = "../crate-b" }
"#,
        )
        .unwrap();

        fs::create_dir_all(root.join("workspace/crate-b")).unwrap();
        fs::write(
            root.join("workspace/crate-b/Cargo.toml"),
            r#"
[package]
name = "crate-b"
"#,
        )
        .unwrap();

        // Create standalone crate
        fs::create_dir_all(root.join("standalone")).unwrap();
        fs::write(
            root.join("standalone/Cargo.toml"),
            r#"
[package]
name = "standalone-crate"
"#,
        )
        .unwrap();
        fs::write(root.join("standalone/Cargo.lock"), "# lock file").unwrap();

        temp
    }

    #[test]
    fn test_discover_workspace_and_standalone() {
        let temp = create_test_workspace();
        let mut discovery = WorkspaceDiscovery::new();

        let roots = discovery
            .discover_all(&[temp.path().to_path_buf()], None)
            .unwrap();

        assert_eq!(roots.len(), 2);

        // Check standalone crate
        let standalone = roots.iter().find(|r| r.is_standalone).unwrap();
        assert_eq!(standalone.name, "standalone-crate");
        assert_eq!(standalone.members.len(), 1);

        // Check workspace
        let workspace = roots.iter().find(|r| !r.is_standalone).unwrap();
        assert_eq!(workspace.name, "workspace");
        assert_eq!(workspace.members.len(), 2);
        assert!(workspace.workspace_dependencies.contains_key("shared"));
    }
}
