//! Deps command configuration

use std::path::PathBuf;

use crate::cli::OutputFormat;

#[derive(Debug, Clone)]
pub struct WorkspaceDepsConfig {
    pub workspace: Option<String>,
    pub reverse: bool,
    pub transitive: bool,
    pub paths: Vec<PathBuf>,
    pub format: OutputFormat,
    pub exclude_dev: bool,
    pub exclude_build: bool,
    pub exclude_target: bool,
}

impl WorkspaceDepsConfig {
    pub fn builder() -> WorkspaceDepsConfigBuilder {
        WorkspaceDepsConfigBuilder::new()
    }
}

#[derive(Default)]
pub struct WorkspaceDepsConfigBuilder {
    workspace: Option<Option<String>>,
    reverse: Option<bool>,
    transitive: Option<bool>,
    paths: Option<Vec<PathBuf>>,
    format: Option<OutputFormat>,
    exclude_dev: Option<bool>,
    exclude_build: Option<bool>,
    exclude_target: Option<bool>,
}

impl WorkspaceDepsConfigBuilder {
    pub fn new() -> Self {
        Self {
            workspace: None,
            reverse: None,
            transitive: None,
            paths: None,
            format: None,
            exclude_dev: None,
            exclude_build: None,
            exclude_target: None,
        }
    }

    pub fn with_workspace(mut self, workspace: Option<String>) -> Self {
        self.workspace = Some(workspace);
        self
    }

    pub fn with_reverse(mut self, reverse: bool) -> Self {
        self.reverse = Some(reverse);
        self
    }

    pub fn with_transitive(mut self, transitive: bool) -> Self {
        self.transitive = Some(transitive);
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
}

impl crate::common::ConfigBuilder for WorkspaceDepsConfigBuilder {
    type Config = WorkspaceDepsConfig;

    fn build(self) -> Result<Self::Config, crate::error::FerrisWheelError> {
        Ok(WorkspaceDepsConfig {
            workspace: self.workspace.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: workspace".to_string(),
                }
            })?,
            reverse: self.reverse.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: reverse".to_string(),
                }
            })?,
            transitive: self.transitive.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: transitive".to_string(),
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
        })
    }
}
