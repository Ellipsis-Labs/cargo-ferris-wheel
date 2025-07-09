//! Dependency filtering functionality

use crate::analyzer::Dependency;

/// Encapsulates dependency filtering logic based on dependency types
#[derive(Debug, Clone, Copy, Default)]
pub struct DependencyFilter {
    exclude_dev: bool,
    exclude_build: bool,
    exclude_target: bool,
}

impl DependencyFilter {
    /// Create a new dependency filter
    pub fn new(exclude_dev: bool, exclude_build: bool, exclude_target: bool) -> Self {
        Self {
            exclude_dev,
            exclude_build,
            exclude_target,
        }
    }

    /// Check if dev dependencies should be included
    pub fn include_dev(&self) -> bool {
        !self.exclude_dev
    }

    /// Check if build dependencies should be included
    pub fn include_build(&self) -> bool {
        !self.exclude_build
    }

    /// Check if target-specific dependencies should be included
    pub fn include_target(&self) -> bool {
        !self.exclude_target
    }

    /// Check if a dependency should be included based on its target field
    ///
    /// This method only filters based on the dependency's target field.
    /// Filtering by dependency type (dev, build) happens at a higher level
    /// where dependencies are already categorized into separate collections.
    pub fn should_include_dependency(&self, dep: &Dependency) -> bool {
        // If it has a target and we're excluding targets, skip it
        if dep.target().is_some() && self.exclude_target {
            return false;
        }
        true
    }
}

impl From<&crate::common::CommonArgs> for DependencyFilter {
    fn from(args: &crate::common::CommonArgs) -> Self {
        Self::new(args.exclude_dev, args.exclude_build, args.exclude_target)
    }
}
