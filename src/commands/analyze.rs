//! Spotlight command implementation

use miette::{Result, WrapErr};

use crate::cli::Commands;
use crate::common::{ConfigBuilder, FromCommand};
use crate::config::AnalyzeCrateConfig;
use crate::error::FerrisWheelError;

impl FromCommand for AnalyzeCrateConfig {
    fn from_command(command: Commands) -> Result<Self, FerrisWheelError> {
        match command {
            Commands::Spotlight {
                crate_name,
                common,
                format,
                cycle_display,
                intra_workspace,
            } => AnalyzeCrateConfig::builder()
                .with_crate_name(crate_name)
                .with_paths(common.get_paths())
                .with_format(format.format)
                .with_exclude_dev(common.exclude_dev)
                .with_exclude_build(common.exclude_build)
                .with_exclude_target(common.exclude_target)
                .with_max_cycles(cycle_display.max_cycles)
                .with_intra_workspace(intra_workspace)
                .build(),
            _ => Err(FerrisWheelError::ConfigurationError {
                message: "Invalid command type for AnalyzeCrateConfig".to_string(),
            }),
        }
    }
}

crate::impl_try_from_command!(AnalyzeCrateConfig);

/// Execute the spotlight command for analyzing cycles involving a specific
/// crate
pub fn execute_analyze_command(command: Commands) -> Result<()> {
    let config = AnalyzeCrateConfig::from_command(command)
        .wrap_err("Failed to parse spotlight command configuration")?;

    use crate::executors::CommandExecutor;
    use crate::executors::analyze::AnalyzeExecutor;
    AnalyzeExecutor::execute(config)
}
