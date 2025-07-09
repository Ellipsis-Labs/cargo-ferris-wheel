//! Graph command configuration

use std::path::PathBuf;

use crate::cli::GraphFormat;
use crate::impl_builder;

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

impl_builder! {
    GraphOptionsBuilder => GraphOptions {
        paths: Vec<PathBuf>,
        format: GraphFormat,
        output: Option<PathBuf>,
        highlight_cycles: bool,
        show_crates: bool,
        exclude_dev: bool,
        exclude_build: bool,
        exclude_target: bool,
    }
}
