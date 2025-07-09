//! Analyze command configuration

use std::path::PathBuf;

use crate::cli::OutputFormat;

#[derive(Debug, Clone)]
pub struct AnalyzeCrateConfig {
    pub crate_name: String,
    pub paths: Vec<PathBuf>,
    pub format: OutputFormat,
    pub exclude_dev: bool,
    pub exclude_build: bool,
    pub exclude_target: bool,
    pub max_cycles: Option<usize>,
    pub intra_workspace: bool,
}

impl AnalyzeCrateConfig {
    pub fn builder() -> AnalyzeCrateConfigBuilder {
        AnalyzeCrateConfigBuilder::new()
    }
}

#[derive(Default)]
pub struct AnalyzeCrateConfigBuilder {
    crate_name: Option<String>,
    paths: Option<Vec<PathBuf>>,
    format: Option<OutputFormat>,
    exclude_dev: Option<bool>,
    exclude_build: Option<bool>,
    exclude_target: Option<bool>,
    max_cycles: Option<Option<usize>>,
    intra_workspace: Option<bool>,
}

impl AnalyzeCrateConfigBuilder {
    pub fn new() -> Self {
        Self {
            crate_name: None,
            paths: None,
            format: None,
            exclude_dev: None,
            exclude_build: None,
            exclude_target: None,
            max_cycles: None,
            intra_workspace: None,
        }
    }

    pub fn with_crate_name(mut self, crate_name: String) -> Self {
        self.crate_name = Some(crate_name);
        self
    }

    pub fn with_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = Some(paths);
        self
    }

    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = Some(format);
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

impl crate::common::ConfigBuilder for AnalyzeCrateConfigBuilder {
    type Config = AnalyzeCrateConfig;

    fn build(self) -> Result<Self::Config, crate::error::FerrisWheelError> {
        Ok(AnalyzeCrateConfig {
            crate_name: self.crate_name.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: crate_name".to_string(),
                }
            })?,
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
