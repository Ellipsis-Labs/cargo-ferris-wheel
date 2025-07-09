//! Configuration for the affected command

use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::error::FerrisWheelError;

#[derive(Debug, Clone)]
pub struct AffectedConfig {
    /// List of changed files
    pub files: Vec<String>,

    /// Include crate-level information
    pub show_crates: bool,

    /// Include only directly affected crates (no reverse dependencies)
    pub direct_only: bool,

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
}

impl AffectedConfig {
    pub fn builder() -> AffectedConfigBuilder {
        AffectedConfigBuilder::default()
    }
}

pub struct AffectedConfigBuilder {
    files: Vec<String>,
    show_crates: bool,
    direct_only: bool,
    paths: Vec<PathBuf>,
    format: OutputFormat,
    exclude_dev: bool,
    exclude_build: bool,
    exclude_target: bool,
}

impl Default for AffectedConfigBuilder {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            show_crates: false,
            direct_only: false,
            paths: Vec::new(),
            format: OutputFormat::Human,
            exclude_dev: false,
            exclude_build: false,
            exclude_target: false,
        }
    }
}

impl AffectedConfigBuilder {
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files = files;
        self
    }

    pub fn with_show_crates(mut self, show: bool) -> Self {
        self.show_crates = show;
        self
    }

    pub fn with_direct_only(mut self, direct: bool) -> Self {
        self.direct_only = direct;
        self
    }

    pub fn with_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = paths;
        self
    }

    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_exclude_dev(mut self, exclude: bool) -> Self {
        self.exclude_dev = exclude;
        self
    }

    pub fn with_exclude_build(mut self, exclude: bool) -> Self {
        self.exclude_build = exclude;
        self
    }

    pub fn with_exclude_target(mut self, exclude: bool) -> Self {
        self.exclude_target = exclude;
        self
    }

    pub fn build(self) -> Result<AffectedConfig, FerrisWheelError> {
        if self.files.is_empty() {
            return Err(FerrisWheelError::ConfigurationError {
                message: "No files specified for affected analysis".to_string(),
            });
        }

        Ok(AffectedConfig {
            files: self.files,
            show_crates: self.show_crates,
            direct_only: self.direct_only,
            paths: self.paths,
            format: self.format,
            exclude_dev: self.exclude_dev,
            exclude_build: self.exclude_build,
            exclude_target: self.exclude_target,
        })
    }
}
