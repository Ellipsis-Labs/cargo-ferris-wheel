//! Command executors that handle the actual logic for each command

pub mod affected;
pub mod analyze;
pub mod check;
pub mod deps;
pub mod graph;

use miette::Result;

/// Trait for command executors
pub trait CommandExecutor {
    type Config;

    /// Execute the command with the given configuration
    fn execute(config: Self::Config) -> Result<()>;
}
