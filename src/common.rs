//! Common functionality shared across commands

use std::path::PathBuf;

use clap::Args;

/// Common arguments shared by multiple commands
#[derive(Args, Debug, Clone)]
pub struct CommonArgs {
    /// Paths to analyze (defaults to current directory)
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Exclude dev-dependencies from analysis
    #[arg(long, env = "CARGO_FERRIS_WHEEL_EXCLUDE_DEV")]
    pub exclude_dev: bool,

    /// Exclude build-dependencies from analysis
    #[arg(long, env = "CARGO_FERRIS_WHEEL_EXCLUDE_BUILD")]
    pub exclude_build: bool,

    /// Exclude target-specific dependencies
    #[arg(long, env = "CARGO_FERRIS_WHEEL_EXCLUDE_TARGET")]
    pub exclude_target: bool,
}

/// Common output format arguments
#[derive(Args, Debug, Clone)]
pub struct FormatArgs {
    /// Output format
    #[arg(short, long, value_enum, default_value = crate::constants::output::DEFAULT_FORMAT, env = "CARGO_FERRIS_WHEEL_FORMAT")]
    pub format: crate::cli::OutputFormat,
}

/// Common cycle display arguments  
#[derive(Args, Debug, Clone)]
pub struct CycleDisplayArgs {
    /// Maximum number of cycles to display (shows all by default)
    #[arg(long, env = "CARGO_FERRIS_WHEEL_MAX_CYCLES")]
    pub max_cycles: Option<usize>,
}

impl CommonArgs {
    /// Get paths, using current directory if none provided
    pub fn get_paths(&self) -> Vec<PathBuf> {
        if self.paths.is_empty() {
            vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
        } else {
            self.paths.clone()
        }
    }
}

/// Generic builder trait for configuration objects
pub trait ConfigBuilder: Sized {
    type Config;

    /// Build the configuration, returning an error if validation fails
    fn build(self) -> Result<Self::Config, crate::error::FerrisWheelError>;
}

/// Trait for configurations that can be created from CLI commands
/// This trait simplifies command-to-config conversions
pub trait FromCommand: Sized {
    /// The command variant that this config can be created from
    fn from_command(command: crate::cli::Commands) -> Result<Self, crate::error::FerrisWheelError>;
}

/// Macro to implement `TryFrom<Commands>` using [`FromCommand`] trait
#[macro_export]
macro_rules! impl_try_from_command {
    ($config:ty) => {
        impl std::convert::TryFrom<$crate::cli::Commands> for $config {
            type Error = $crate::error::FerrisWheelError;

            fn try_from(command: $crate::cli::Commands) -> Result<Self, Self::Error> {
                <$config as $crate::common::FromCommand>::from_command(command)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_args_get_paths_empty() {
        let args = CommonArgs {
            paths: vec![],
            exclude_dev: false,
            exclude_build: false,
            exclude_target: false,
        };

        let paths = args.get_paths();
        assert_eq!(paths.len(), 1);
        // Should default to current directory
        assert!(paths[0].is_absolute() || paths[0] == std::path::Path::new("."));
    }

    #[test]
    fn test_common_args_get_paths_with_values() {
        let test_paths = vec![PathBuf::from("/tmp/test1"), PathBuf::from("/tmp/test2")];

        let args = CommonArgs {
            paths: test_paths.clone(),
            exclude_dev: false,
            exclude_build: false,
            exclude_target: false,
        };

        let paths = args.get_paths();
        assert_eq!(paths, test_paths);
    }
}
