//! Command implementations for ferris-wheel CLI
//!
//! This module contains the implementations for each CLI command:
//! - inspect: Inspect the carnival rides for dangerous cycles
//! - spotlight: Put a spotlight on cycles involving a specific crate
//! - lineup: See the full lineup of workspace dependencies
//! - spectacle: Create a spectacular visualization of dependencies
//! - ripples: Discover the ripple effects from changed files

pub mod affected;
pub mod analyze;
pub mod check;
pub mod deps;
pub mod graph;

use miette::Result;

use crate::cli::Commands;

/// Execute a command based on CLI input
pub fn execute_command(command: Commands) -> Result<()> {
    match &command {
        Commands::Inspect { .. } => check::execute_check_command(command),
        Commands::Spectacle { .. } => graph::execute_graph_command(command),
        Commands::Spotlight { .. } => analyze::execute_analyze_command(command),
        Commands::Lineup { .. } => deps::execute_deps_command(command),
        Commands::Ripples { .. } => affected::execute_affected_command(command),
    }
}
