//! Deps command configuration

use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::impl_builder;

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

impl_builder! {
    WorkspaceDepsConfigBuilder => WorkspaceDepsConfig {
        workspace: Option<String>,
        reverse: bool,
        transitive: bool,
        paths: Vec<PathBuf>,
        format: OutputFormat,
        exclude_dev: bool,
        exclude_build: bool,
        exclude_target: bool,
    }
}
