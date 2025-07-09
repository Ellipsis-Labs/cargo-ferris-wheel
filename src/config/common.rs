//! Common configuration structures shared across commands

use crate::cli::OutputFormat;
use std::path::PathBuf;

/// Common analysis configuration shared by multiple commands
#[derive(Debug, Clone)]
pub struct CommonAnalysisConfig {
    /// Paths to analyze
    pub paths: Vec<PathBuf>,
    /// Output format
    pub format: OutputFormat,
    /// Exclude dev-dependencies from analysis
    pub exclude_dev: bool,
    /// Exclude build-dependencies from analysis
    pub exclude_build: bool,
    /// Exclude target-specific dependencies
    pub exclude_target: bool,
    /// Maximum number of cycles to display
    pub max_cycles: Option<usize>,
    /// Check for cycles within workspaces (intra-workspace)
    pub intra_workspace: bool,
}

impl CommonAnalysisConfig {
    /// Create a new CommonAnalysisConfig with default values
    pub fn new(paths: Vec<PathBuf>, format: OutputFormat) -> Self {
        Self {
            paths,
            format,
            exclude_dev: false,
            exclude_build: false,
            exclude_target: false,
            max_cycles: None,
            intra_workspace: false,
        }
    }

    /// Create a builder for CommonAnalysisConfig
    pub fn builder() -> CommonAnalysisConfigBuilder {
        CommonAnalysisConfigBuilder::default()
    }
}

/// Builder for CommonAnalysisConfig
#[derive(Default)]
pub struct CommonAnalysisConfigBuilder {
    paths: Option<Vec<PathBuf>>,
    format: Option<OutputFormat>,
    exclude_dev: bool,
    exclude_build: bool,
    exclude_target: bool,
    max_cycles: Option<usize>,
    intra_workspace: bool,
}

impl CommonAnalysisConfigBuilder {
    pub fn paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = Some(paths);
        self
    }

    pub fn format(mut self, format: OutputFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn exclude_dev(mut self, exclude_dev: bool) -> Self {
        self.exclude_dev = exclude_dev;
        self
    }

    pub fn exclude_build(mut self, exclude_build: bool) -> Self {
        self.exclude_build = exclude_build;
        self
    }

    pub fn exclude_target(mut self, exclude_target: bool) -> Self {
        self.exclude_target = exclude_target;
        self
    }

    pub fn max_cycles(mut self, max_cycles: Option<usize>) -> Self {
        self.max_cycles = max_cycles;
        self
    }

    pub fn intra_workspace(mut self, intra_workspace: bool) -> Self {
        self.intra_workspace = intra_workspace;
        self
    }

    pub fn build(self) -> Result<CommonAnalysisConfig, crate::error::FerrisWheelError> {
        Ok(CommonAnalysisConfig {
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
            exclude_dev: self.exclude_dev,
            exclude_build: self.exclude_build,
            exclude_target: self.exclude_target,
            max_cycles: self.max_cycles,
            intra_workspace: self.intra_workspace,
        })
    }
}
