//! Check command configuration

use std::path::PathBuf;

use crate::cli::OutputFormat;

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

#[derive(Default)]
pub struct CheckCyclesConfigBuilder {
    paths: Option<Vec<PathBuf>>,
    format: Option<OutputFormat>,
    error_on_cycles: Option<bool>,
    exclude_dev: Option<bool>,
    exclude_build: Option<bool>,
    exclude_target: Option<bool>,
    max_cycles: Option<Option<usize>>,
    intra_workspace: Option<bool>,
}

impl CheckCyclesConfigBuilder {
    pub fn new() -> Self {
        Self {
            paths: None,
            format: None,
            error_on_cycles: None,
            exclude_dev: None,
            exclude_build: None,
            exclude_target: None,
            max_cycles: None,
            intra_workspace: None,
        }
    }

    pub fn with_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = Some(paths);
        self
    }

    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn with_error_on_cycles(mut self, error_on_cycles: bool) -> Self {
        self.error_on_cycles = Some(error_on_cycles);
        self
    }

    pub fn with_exclude_dev(mut self, exclude_dev: bool) -> Self {
        self.exclude_dev = Some(exclude_dev);
        self
    }

    pub fn with_exclude_build(mut self, exclude_build: bool) -> Self {
        self.exclude_build = Some(exclude_build);
        self
    }

    pub fn with_exclude_target(mut self, exclude_target: bool) -> Self {
        self.exclude_target = Some(exclude_target);
        self
    }

    pub fn with_max_cycles(mut self, max_cycles: Option<usize>) -> Self {
        self.max_cycles = Some(max_cycles);
        self
    }

    pub fn with_intra_workspace(mut self, intra_workspace: bool) -> Self {
        self.intra_workspace = Some(intra_workspace);
        self
    }
}

impl crate::common::ConfigBuilder for CheckCyclesConfigBuilder {
    type Config = CheckCyclesConfig;

    fn build(self) -> Result<Self::Config, crate::error::FerrisWheelError> {
        Ok(CheckCyclesConfig {
            paths: self.paths.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: paths".to_string(),
                }
            })?,
            format: self.format.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: format".to_string(),
                }
            })?,
            error_on_cycles: self.error_on_cycles.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: error_on_cycles".to_string(),
                }
            })?,
            exclude_dev: self.exclude_dev.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: exclude_dev".to_string(),
                }
            })?,
            exclude_build: self.exclude_build.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: exclude_build".to_string(),
                }
            })?,
            exclude_target: self.exclude_target.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: exclude_target".to_string(),
                }
            })?,
            max_cycles: self.max_cycles.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: max_cycles".to_string(),
                }
            })?,
            intra_workspace: self.intra_workspace.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: intra_workspace".to_string(),
                }
            })?,
        })
    }
}
