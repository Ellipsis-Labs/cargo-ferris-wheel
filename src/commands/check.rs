//! Inspect command implementation

use miette::{Result, WrapErr};

use crate::cli::Commands;
use crate::common::{ConfigBuilder, FromCommand};
use crate::config::CheckCyclesConfig;
use crate::error::FerrisWheelError;

impl FromCommand for CheckCyclesConfig {
    fn from_command(command: Commands) -> Result<Self, FerrisWheelError> {
        match command {
            Commands::Inspect {
                common,
                format,
                cycle_display,
                error_on_cycles,
                intra_workspace,
            } => CheckCyclesConfig::builder()
                .with_paths(common.get_paths())
                .with_format(format.format)
                .with_error_on_cycles(error_on_cycles)
                .with_exclude_dev(common.exclude_dev)
                .with_exclude_build(common.exclude_build)
                .with_exclude_target(common.exclude_target)
                .with_max_cycles(cycle_display.max_cycles)
                .with_intra_workspace(intra_workspace)
                .build(),
            _ => Err(FerrisWheelError::ConfigurationError {
                message: "Invalid command type for CheckCyclesConfig".to_string(),
            }),
        }
    }
}

crate::impl_try_from_command!(CheckCyclesConfig);

/// Execute the inspect command for detecting workspace dependency cycles
pub fn execute_check_command(command: Commands) -> Result<()> {
    let config = CheckCyclesConfig::from_command(command)
        .wrap_err("Failed to parse inspect command configuration")?;

    use crate::executors::CommandExecutor;
    use crate::executors::check::CheckExecutor;
    CheckExecutor::execute(config)
}
