//! Spectacle command implementation

use miette::{Result, WrapErr};

use crate::cli::Commands;
use crate::common::{ConfigBuilder, FromCommand};
use crate::config::GraphOptions;
use crate::error::FerrisWheelError;

impl FromCommand for GraphOptions {
    fn from_command(command: Commands) -> Result<Self, FerrisWheelError> {
        match command {
            Commands::Spectacle {
                common,
                format,
                output,
                highlight_cycles,
                show_crates,
            } => GraphOptions::builder()
                .with_paths(common.get_paths())
                .with_format(format)
                .with_output(output)
                .with_highlight_cycles(highlight_cycles)
                .with_show_crates(show_crates)
                .with_exclude_dev(common.exclude_dev)
                .with_exclude_build(common.exclude_build)
                .with_exclude_target(common.exclude_target)
                .build(),
            _ => Err(FerrisWheelError::ConfigurationError {
                message: "Invalid command type for GraphOptions".to_string(),
            }),
        }
    }
}

crate::impl_try_from_command!(GraphOptions);

/// Execute the spectacle command for generating visual dependency graphs
pub fn execute_graph_command(command: Commands) -> Result<()> {
    let config = GraphOptions::from_command(command)
        .wrap_err("Failed to parse spectacle command configuration")?;

    use crate::executors::CommandExecutor;
    use crate::executors::graph::GraphExecutor;
    GraphExecutor::execute(config)
}
