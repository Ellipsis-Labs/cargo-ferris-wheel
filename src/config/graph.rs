//! Graph command configuration

use std::path::PathBuf;

use crate::cli::GraphFormat;

#[derive(Debug, Clone)]
pub struct GraphOptions {
    pub paths: Vec<PathBuf>,
    pub format: GraphFormat,
    pub output: Option<PathBuf>,
    pub highlight_cycles: bool,
    pub show_crates: bool,
    pub exclude_dev: bool,
    pub exclude_build: bool,
    pub exclude_target: bool,
}

impl GraphOptions {
    pub fn builder() -> GraphOptionsBuilder {
        GraphOptionsBuilder::new()
    }
}

#[derive(Default)]
pub struct GraphOptionsBuilder {
    paths: Option<Vec<PathBuf>>,
    format: Option<GraphFormat>,
    output: Option<Option<PathBuf>>,
    highlight_cycles: Option<bool>,
    show_crates: Option<bool>,
    exclude_dev: Option<bool>,
    exclude_build: Option<bool>,
    exclude_target: Option<bool>,
}

impl GraphOptionsBuilder {
    pub fn new() -> Self {
        Self {
            paths: None,
            format: None,
            output: None,
            highlight_cycles: None,
            show_crates: None,
            exclude_dev: None,
            exclude_build: None,
            exclude_target: None,
        }
    }

    pub fn with_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = Some(paths);
        self
    }

    pub fn with_format(mut self, format: GraphFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn with_output(mut self, output: Option<PathBuf>) -> Self {
        self.output = Some(output);
        self
    }

    pub fn with_highlight_cycles(mut self, highlight_cycles: bool) -> Self {
        self.highlight_cycles = Some(highlight_cycles);
        self
    }

    pub fn with_show_crates(mut self, show_crates: bool) -> Self {
        self.show_crates = Some(show_crates);
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

impl crate::common::ConfigBuilder for GraphOptionsBuilder {
    type Config = GraphOptions;

    fn build(self) -> Result<Self::Config, crate::error::FerrisWheelError> {
        Ok(GraphOptions {
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
            output: self.output.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: output".to_string(),
                }
            })?,
            highlight_cycles: self.highlight_cycles.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: highlight_cycles".to_string(),
                }
            })?,
            show_crates: self.show_crates.ok_or_else(|| {
                crate::error::FerrisWheelError::ConfigurationError {
                    message: "Missing required field: show_crates".to_string(),
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
