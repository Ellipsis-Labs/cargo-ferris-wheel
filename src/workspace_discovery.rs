use std::collections::HashSet;
use std::path::{Path, PathBuf};

use miette::{Result, WrapErr};
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::progress::ProgressReporter;
use crate::toml_parser::CargoToml;

pub struct WorkspaceDiscovery {
    discovered_roots: HashSet<PathBuf>,
}

impl WorkspaceDiscovery {
    pub fn new() -> Self {
        Self {
            discovered_roots: HashSet::new(),
        }
    }

    /// Discover all workspace roots and standalone crates in the given paths
    pub fn discover_all(
        &mut self,
        paths: &[PathBuf],
        progress: Option<&ProgressReporter>,
    ) -> Result<Vec<WorkspaceRoot>> {
        let mut roots = Vec::new();

        for path in paths {
            if !path.exists() {
                eprintln!(
                    "{} Path '{}' does not exist",
                    console::style("⚠").yellow(),
                    path.display()
                );
                continue;
            }

            if !path.is_dir() {
                eprintln!(
                    "{} Path '{}' is not a directory",
                    console::style("⚠").yellow(),
                    path.display()
                );
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
        let new_roots: Vec<WorkspaceRoot> = unique_dirs
            .into_par_iter()
            .filter_map(|dir| {
                let cargo_toml_path = dir.join("Cargo.toml");
                if !cargo_toml_path.exists() {
                    return None;
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
                            Some(WorkspaceRoot {
                                path: dir.clone(),
                                name: dir
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string(),
                                members: Vec::new(), // Will be populated later
                                workspace_dependencies: cargo_toml.get_workspace_dependencies(),
                                is_standalone: false,
                            })
                        } else if let Some(ref package) = cargo_toml.package {
                            // This is a standalone crate
                            Some(WorkspaceRoot {
                                path: dir.clone(),
                                name: package.name.clone(),
                                members: vec![WorkspaceMember {
                                    path: dir,
                                    name: package.name.clone(),
                                    cargo_toml,
                                }],
                                workspace_dependencies: Default::default(),
                                is_standalone: true,
                            })
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "{} Failed to parse {}: {}",
                            console::style("⚠").yellow(),
                            cargo_toml_path.display(),
                            e
                        );
                        None
                    }
                }
            })
            .collect();

        // Expand workspace members for workspace roots
        for mut root in new_roots {
            if !root.is_standalone && root.members.is_empty() {
                let cargo_toml_path = root.path.join("Cargo.toml");
                if let Ok(cargo_toml) = CargoToml::parse_file(&cargo_toml_path) {
                    if let Ok(members) = self.expand_workspace_members(&root.path, &cargo_toml) {
                        root.members = members;
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

            if let Ok(cargo_toml) = CargoToml::parse_file(cargo_toml_path) {
                if cargo_toml.is_workspace_root() {
                    self.discovered_roots.insert(dir.to_path_buf());
                    let members = self.expand_workspace_members(dir, &cargo_toml)?;
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
            }
        }

        Ok(())
    }

    fn expand_workspace_members(
        &self,
        workspace_root: &Path,
        cargo_toml: &CargoToml,
    ) -> Result<Vec<WorkspaceMember>> {
        let mut members = Vec::new();
        let member_patterns = cargo_toml.get_workspace_members();

        // Parallelize member expansion
        let new_members: Vec<WorkspaceMember> = member_patterns
            .into_par_iter()
            .flat_map(|pattern| {
                // Handle glob patterns
                if pattern.contains('*') {
                    let glob_pattern = workspace_root.join(&pattern);
                    let glob_str = glob_pattern.to_string_lossy();

                    match glob::glob(&glob_str) {
                        Ok(paths) => {
                            let member_paths: Vec<PathBuf> =
                                paths.flatten().filter(|path| path.is_dir()).collect();

                            member_paths
                                .into_par_iter()
                                .filter_map(|path| match self.load_member_single(&path) {
                                    Ok(Some(member)) => Some(member),
                                    Ok(None) => None,
                                    Err(e) => {
                                        eprintln!(
                                            "{} Failed to load member {}: {}",
                                            console::style("⚠").yellow(),
                                            path.display(),
                                            e
                                        );
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                        }
                        Err(e) => {
                            eprintln!(
                                "{} Invalid glob pattern '{}': {}",
                                console::style("⚠").yellow(),
                                pattern,
                                e
                            );
                            vec![]
                        }
                    }
                } else {
                    // Direct path
                    let member_path = workspace_root.join(&pattern);
                    if member_path.is_dir() {
                        match self.load_member_single(&member_path) {
                            Ok(Some(member)) => vec![member],
                            Ok(None) => vec![],
                            Err(e) => {
                                eprintln!(
                                    "{} Failed to load member {}: {}",
                                    console::style("⚠").yellow(),
                                    member_path.display(),
                                    e
                                );
                                vec![]
                            }
                        }
                    } else {
                        vec![]
                    }
                }
            })
            .collect();

        members.extend(new_members);

        Ok(members)
    }

    fn load_member_single(&self, path: &Path) -> Result<Option<WorkspaceMember>> {
        let cargo_toml_path = path.join("Cargo.toml");
        if cargo_toml_path.exists() {
            match CargoToml::parse_file(&cargo_toml_path) {
                Ok(cargo_toml) => {
                    if let Some(package) = &cargo_toml.package {
                        Ok(Some(WorkspaceMember {
                            path: path.to_path_buf(),
                            name: package.name.clone(),
                            cargo_toml,
                        }))
                    } else {
                        Ok(None)
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{} Failed to parse member {}: {}",
                        console::style("⚠").yellow(),
                        cargo_toml_path.display(),
                        e
                    );
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceRoot {
    pub path: PathBuf,
    pub name: String,
    pub members: Vec<WorkspaceMember>,
    pub workspace_dependencies: std::collections::HashMap<String, PathBuf>,
    pub is_standalone: bool,
}

#[derive(Debug, Clone)]
pub struct WorkspaceMember {
    pub path: PathBuf,
    pub name: String,
    pub cargo_toml: CargoToml,
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
