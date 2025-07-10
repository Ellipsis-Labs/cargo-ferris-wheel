use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::{IntoDiagnostic, NamedSource, Result, SourceSpan};
use serde::Deserialize;

use crate::error::FerrisWheelError;

#[derive(Debug, Clone, Deserialize)]
pub struct CargoToml {
    pub package: Option<Package>,
    pub workspace: Option<Workspace>,
    pub dependencies: Option<HashMap<String, Dependency>>,
    #[serde(rename = "dev-dependencies")]
    pub dev_dependencies: Option<HashMap<String, Dependency>>,
    #[serde(rename = "build-dependencies")]
    pub build_dependencies: Option<HashMap<String, Dependency>>,
    pub target: Option<HashMap<String, TargetDependencies>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Workspace {
    pub members: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    #[serde(rename = "package")]
    pub workspace_package: Option<WorkspacePackage>,
    pub dependencies: Option<HashMap<String, Dependency>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspacePackage {
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetDependencies {
    pub dependencies: Option<HashMap<String, Dependency>>,
    #[serde(rename = "dev-dependencies")]
    pub dev_dependencies: Option<HashMap<String, Dependency>>,
    #[serde(rename = "build-dependencies")]
    pub build_dependencies: Option<HashMap<String, Dependency>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Simple(String),
    Detailed(DetailedDependency),
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetailedDependency {
    pub path: Option<String>,
    pub workspace: Option<bool>,
    pub version: Option<String>,
    pub features: Option<Vec<String>>,
    pub default_features: Option<bool>,
    pub optional: Option<bool>,
}

impl CargoToml {
    pub fn parse_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| FerrisWheelError::FileReadError {
                path: path.to_path_buf(),
                source: e,
            })
            .into_diagnostic()?;

        toml::from_str(&content)
            .map_err(|e| {
                // Try to extract span information from the error
                let span = e
                    .span()
                    .map(|span| SourceSpan::new(span.start.into(), span.end - span.start));

                FerrisWheelError::TomlParseError(Box::new(crate::error::TomlParseError {
                    file: path.display().to_string(),
                    source_code: NamedSource::new(path.display().to_string(), content.clone()),
                    span,
                    source: e,
                }))
            })
            .into_diagnostic()
    }

    pub fn is_workspace_root(&self) -> bool {
        self.workspace.is_some() && self.package.is_none()
    }

    pub fn get_workspace_members(&self) -> Vec<String> {
        self.workspace
            .as_ref()
            .and_then(|ws| ws.members.as_ref())
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_workspace_excludes(&self) -> Vec<String> {
        self.workspace
            .as_ref()
            .and_then(|ws| ws.exclude.as_ref())
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_workspace_dependencies(&self) -> HashMap<String, PathBuf> {
        let mut deps = HashMap::new();

        if let Some(workspace) = &self.workspace
            && let Some(workspace_deps) = &workspace.dependencies
        {
            for (name, dep) in workspace_deps {
                if let Some(path) = Self::extract_path(dep) {
                    deps.insert(name.clone(), PathBuf::from(path));
                }
            }
        }

        deps
    }

    pub fn get_all_dependencies(&self) -> Vec<(String, Dependency, DependencyType)> {
        let mut all_deps = Vec::new();

        // Normal dependencies
        if let Some(deps) = &self.dependencies {
            for (name, dep) in deps {
                all_deps.push((name.clone(), dep.clone(), DependencyType::Normal));
            }
        }

        // Dev dependencies
        if let Some(deps) = &self.dev_dependencies {
            for (name, dep) in deps {
                all_deps.push((name.clone(), dep.clone(), DependencyType::Dev));
            }
        }

        // Build dependencies
        if let Some(deps) = &self.build_dependencies {
            for (name, dep) in deps {
                all_deps.push((name.clone(), dep.clone(), DependencyType::Build));
            }
        }

        // Target-specific dependencies
        if let Some(targets) = &self.target {
            for (target_name, target_deps) in targets {
                if let Some(deps) = &target_deps.dependencies {
                    for (name, dep) in deps {
                        all_deps.push((
                            name.clone(),
                            dep.clone(),
                            DependencyType::Target(target_name.clone()),
                        ));
                    }
                }
                if let Some(deps) = &target_deps.dev_dependencies {
                    for (name, dep) in deps {
                        all_deps.push((
                            name.clone(),
                            dep.clone(),
                            DependencyType::TargetDev(target_name.clone()),
                        ));
                    }
                }
                if let Some(deps) = &target_deps.build_dependencies {
                    for (name, dep) in deps {
                        all_deps.push((
                            name.clone(),
                            dep.clone(),
                            DependencyType::TargetBuild(target_name.clone()),
                        ));
                    }
                }
            }
        }

        all_deps
    }

    pub fn extract_path(dep: &Dependency) -> Option<String> {
        match dep {
            Dependency::Simple(_) => None,
            Dependency::Detailed(detailed) => detailed.path.clone(),
        }
    }

    pub fn is_workspace_dependency(dep: &Dependency) -> bool {
        match dep {
            Dependency::Simple(_) => false,
            Dependency::Detailed(detailed) => detailed.workspace.unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DependencyType {
    Normal,
    Dev,
    Build,
    Target(String),
    TargetDev(String),
    TargetBuild(String),
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_parse_workspace_root() {
        let toml_content = r#"
[workspace]
members = ["crate-a", "crate-b"]
exclude = ["ignored"]

[workspace.dependencies]
atlas-sdk = { path = "../sdk/sdk" }
serde = "1.0"
"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(toml_content.as_bytes()).unwrap();

        let cargo_toml = CargoToml::parse_file(file.path()).unwrap();

        assert!(cargo_toml.is_workspace_root());
        assert_eq!(
            cargo_toml.get_workspace_members(),
            vec!["crate-a", "crate-b"]
        );

        let workspace_deps = cargo_toml.get_workspace_dependencies();
        assert_eq!(
            workspace_deps.get("atlas-sdk"),
            Some(&PathBuf::from("../sdk/sdk"))
        );
        assert_eq!(workspace_deps.get("serde"), None); // No path
    }

    #[test]
    fn test_parse_crate_with_dependencies() {
        let toml_content = r#"
[package]
name = "my-crate"

[dependencies]
atlas-core = { path = "../core" }
serde = { workspace = true }
tokio = "1.0"

[dev-dependencies]
test-utils = { path = "./test-utils" }
"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(toml_content.as_bytes()).unwrap();

        let cargo_toml = CargoToml::parse_file(file.path()).unwrap();

        assert!(!cargo_toml.is_workspace_root());
        assert_eq!(cargo_toml.package.as_ref().unwrap().name, "my-crate");

        let all_deps = cargo_toml.get_all_dependencies();
        assert_eq!(all_deps.len(), 4);

        // Check path extraction
        let atlas_core_dep = &all_deps
            .iter()
            .find(|(name, _, _)| name == "atlas-core")
            .unwrap()
            .1;
        assert_eq!(
            CargoToml::extract_path(atlas_core_dep),
            Some("../core".to_string())
        );

        // Check workspace dependency
        let serde_dep = &all_deps
            .iter()
            .find(|(name, _, _)| name == "serde")
            .unwrap()
            .1;
        assert!(CargoToml::is_workspace_dependency(serde_dep));
    }
}
