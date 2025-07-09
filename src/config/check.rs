//! Check command configuration

use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::impl_builder;

/// Configuration for the check command
///
/// This struct contains all options for detecting and reporting dependency
/// cycles in Rust workspaces.
#[derive(Debug, Clone)]
pub struct CheckCyclesConfig {
    /// Paths to search for Cargo workspaces
    pub paths: Vec<PathBuf>,
    /// Output format for the report
    pub format: OutputFormat,
    /// Whether to exit with error code if cycles are found
    pub error_on_cycles: bool,
    /// Exclude dev dependencies from cycle detection
    pub exclude_dev: bool,
    /// Exclude build dependencies from cycle detection
    pub exclude_build: bool,
    /// Exclude target-specific dependencies from cycle detection
    pub exclude_target: bool,
    /// Maximum number of cycles to report (None = all)
    pub max_cycles: Option<usize>,
    /// Only check for cycles within each workspace (not across workspaces)
    pub intra_workspace: bool,
}

impl CheckCyclesConfig {
    pub fn builder() -> CheckCyclesConfigBuilder {
        CheckCyclesConfigBuilder::new()
    }
}

impl_builder! {
    CheckCyclesConfigBuilder => CheckCyclesConfig {
        paths: Vec<PathBuf>,
        format: OutputFormat,
        error_on_cycles: bool,
        exclude_dev: bool,
        exclude_build: bool,
        exclude_target: bool,
        max_cycles: Option<usize>,
        intra_workspace: bool,
    }
}
