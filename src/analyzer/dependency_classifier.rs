//! Dependency classification logic extracted from analyzer
//!
//! This module handles the classification of dependencies into their
//! appropriate types (normal, dev, build, target-specific) based on the TOML
//! dependency information.

use std::collections::HashMap;

use crate::analyzer::{Dependency, DependencyBuilderError};
use crate::toml_parser::{
    CargoToml, Dependency as TomlDependency, DependencyType as TomlDependencyType,
};

/// Classifies dependencies from a parsed Cargo.toml into categorized vectors
pub struct DependencyClassifier {
    dependencies: Vec<Dependency>,
    dev_dependencies: Vec<Dependency>,
    build_dependencies: Vec<Dependency>,
    target_dependencies: HashMap<String, Vec<Dependency>>,
}

impl DependencyClassifier {
    /// Gets the normal dependencies
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    /// Gets the dev dependencies
    pub fn dev_dependencies(&self) -> &[Dependency] {
        &self.dev_dependencies
    }

    /// Gets the build dependencies
    pub fn build_dependencies(&self) -> &[Dependency] {
        &self.build_dependencies
    }

    /// Gets the target-specific dependencies
    pub fn target_dependencies(&self) -> &HashMap<String, Vec<Dependency>> {
        &self.target_dependencies
    }

    /// Gets mutable access to normal dependencies
    pub fn dependencies_mut(&mut self) -> &mut Vec<Dependency> {
        &mut self.dependencies
    }

    /// Gets mutable access to dev dependencies
    pub fn dev_dependencies_mut(&mut self) -> &mut Vec<Dependency> {
        &mut self.dev_dependencies
    }

    /// Gets mutable access to build dependencies
    pub fn build_dependencies_mut(&mut self) -> &mut Vec<Dependency> {
        &mut self.build_dependencies
    }

    /// Gets mutable access to target-specific dependencies
    pub fn target_dependencies_mut(&mut self) -> &mut HashMap<String, Vec<Dependency>> {
        &mut self.target_dependencies
    }
}

impl Default for DependencyClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyClassifier {
    /// Create a new empty classifier
    pub fn new() -> Self {
        Self {
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            build_dependencies: Vec::new(),
            target_dependencies: HashMap::new(),
        }
    }

    /// Classify dependencies from a CargoToml
    pub fn classify_from_toml(
        cargo_toml: &CargoToml,
        workspace_deps: &HashMap<String, std::path::PathBuf>,
    ) -> Self {
        let mut classifier = Self::new();

        for (dep_name, dep, dep_type) in cargo_toml.get_all_dependencies() {
            if !Self::is_relevant_dependency(&dep_name, &dep, workspace_deps) {
                continue;
            }

            if let Ok(dependency) = Self::create_dependency(&dep_name, &dep_type) {
                classifier.add_dependency(dependency, dep_type);
            }
        }

        classifier
    }

    /// Check if a dependency is relevant (i.e., is a path or workspace
    /// dependency)
    fn is_relevant_dependency(
        dep_name: &str,
        dep: &TomlDependency,
        workspace_deps: &HashMap<String, std::path::PathBuf>,
    ) -> bool {
        if CargoToml::is_workspace_dependency(dep) {
            workspace_deps.contains_key(dep_name)
        } else {
            CargoToml::extract_path(dep).is_some()
        }
    }

    /// Create a Dependency struct from name and type
    fn create_dependency(
        dep_name: &str,
        dep_type: &TomlDependencyType,
    ) -> Result<Dependency, DependencyBuilderError> {
        let mut builder = Dependency::builder().with_name(dep_name);

        match dep_type {
            TomlDependencyType::Target(t)
            | TomlDependencyType::TargetDev(t)
            | TomlDependencyType::TargetBuild(t) => {
                builder = builder.with_target(t.to_string());
            }
            _ => {}
        }

        builder.build()
    }

    /// Add a dependency to the appropriate collection based on its type
    fn add_dependency(&mut self, dependency: Dependency, dep_type: TomlDependencyType) {
        match dep_type {
            TomlDependencyType::Normal => {
                self.dependencies.push(dependency);
            }
            TomlDependencyType::Dev => {
                self.dev_dependencies.push(dependency);
            }
            TomlDependencyType::Build => {
                self.build_dependencies.push(dependency);
            }
            TomlDependencyType::Target(target) => {
                self.target_dependencies
                    .entry(target)
                    .or_default()
                    .push(dependency);
            }
            TomlDependencyType::TargetDev(_) | TomlDependencyType::TargetBuild(_) => {
                // Treat target-specific dev/build dependencies as regular target dependencies
                if let Some(target) = dependency.target().map(|t| t.to_string()) {
                    self.target_dependencies
                        .entry(target)
                        .or_default()
                        .push(dependency);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_classifier() {
        let classifier = DependencyClassifier::new();
        assert!(classifier.dependencies.is_empty());
        assert!(classifier.dev_dependencies.is_empty());
        assert!(classifier.build_dependencies.is_empty());
        assert!(classifier.target_dependencies.is_empty());
    }

    #[test]
    fn test_create_dependency_normal() {
        let dep =
            DependencyClassifier::create_dependency("test-crate", &TomlDependencyType::Normal)
                .expect("Failed to create dependency");
        assert_eq!(dep.name(), "test-crate");
        assert!(dep.target().is_none());
    }

    #[test]
    fn test_create_dependency_with_target() {
        let dep = DependencyClassifier::create_dependency(
            "test-crate",
            &TomlDependencyType::Target("wasm32-unknown-unknown".to_string()),
        )
        .expect("Failed to create dependency");
        assert_eq!(dep.name(), "test-crate");
        assert_eq!(dep.target(), Some("wasm32-unknown-unknown"));
    }

    #[test]
    fn test_add_dependencies() {
        let mut classifier = DependencyClassifier::new();

        // Add normal dependency
        let normal_dep = Dependency::builder()
            .with_name("normal-dep")
            .build()
            .expect("Failed to create dependency");
        classifier.add_dependency(normal_dep, TomlDependencyType::Normal);
        assert_eq!(classifier.dependencies.len(), 1);

        // Add dev dependency
        let dev_dep = Dependency::builder()
            .with_name("dev-dep")
            .build()
            .expect("Failed to create dependency");
        classifier.add_dependency(dev_dep, TomlDependencyType::Dev);
        assert_eq!(classifier.dev_dependencies.len(), 1);

        // Add build dependency
        let build_dep = Dependency::builder()
            .with_name("build-dep")
            .build()
            .expect("Failed to create dependency");
        classifier.add_dependency(build_dep, TomlDependencyType::Build);
        assert_eq!(classifier.build_dependencies.len(), 1);

        // Add target dependency
        let target_dep = Dependency::builder()
            .with_name("target-dep")
            .with_target("wasm32-unknown-unknown")
            .build()
            .expect("Failed to create dependency");
        classifier.add_dependency(
            target_dep,
            TomlDependencyType::Target("wasm32-unknown-unknown".to_string()),
        );
        assert_eq!(classifier.target_dependencies.len(), 1);
        assert_eq!(
            classifier.target_dependencies["wasm32-unknown-unknown"].len(),
            1
        );
    }
}
