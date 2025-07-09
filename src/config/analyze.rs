//! Analyze command configuration

use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::impl_builder;

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

impl_builder! {
    AnalyzeCrateConfigBuilder => AnalyzeCrateConfig {
        crate_name: String,
        paths: Vec<PathBuf>,
        format: OutputFormat,
        exclude_dev: bool,
        exclude_build: bool,
        exclude_target: bool,
        max_cycles: Option<usize>,
        intra_workspace: bool,
    }
}
