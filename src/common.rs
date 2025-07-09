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

/// Macro to implement TryFrom<Commands> using FromCommand trait
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

/// Macro to generate builder implementations
#[macro_export]
macro_rules! impl_builder {
    (
        $builder:ident => $config:ident {
            $(
                $field:ident: $type:ty
            ),* $(,)?
        }
    ) => {
        pub struct $builder {
            $(
                $field: Option<$type>,
            )*
        }

        impl Default for $builder {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $builder {
            pub fn new() -> Self {
                Self {
                    $(
                        $field: None,
                    )*
                }
            }

            $(
                pastey::paste! {
                    pub fn [<with_ $field>](mut self, $field: $type) -> Self {
                        self.$field = Some($field);
                        self
                    }
                }
            )*
        }

        impl $crate::common::ConfigBuilder for $builder {
            type Config = $config;

            fn build(self) -> Result<Self::Config, $crate::error::FerrisWheelError> {
                Ok($config {
                    $(
                        $field: self.$field.ok_or_else(|| {
                            $crate::error::FerrisWheelError::ConfigurationError {
                                message: format!("Missing required field: {}", stringify!($field)),
                            }
                        })?,
                    )*
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::impl_builder;

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
        assert!(paths[0].is_absolute() || paths[0] == PathBuf::from("."));
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

    #[test]
    fn test_builder_macro_simple() {
        // Test struct for the builder macro
        #[derive(Debug, PartialEq)]
        pub struct TestConfig {
            name: String,
            value: i32,
        }

        impl_builder! {
            TestConfigBuilder => TestConfig {
                name: String,
                value: i32,
            }
        }

        // Test successful build
        let config = TestConfigBuilder::new()
            .with_name("test".to_string())
            .with_value(42)
            .build()
            .unwrap();

        assert_eq!(config.name, "test");
        assert_eq!(config.value, 42);
    }

    #[test]
    fn test_builder_macro_missing_field() {
        #[derive(Debug, PartialEq)]
        pub struct TestConfig {
            name: String,
            value: i32,
        }

        impl_builder! {
            TestConfigBuilder => TestConfig {
                name: String,
                value: i32,
            }
        }

        // Test missing 'value' field error
        let result = TestConfigBuilder::new()
            .with_name("test".to_string())
            .build();

        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::FerrisWheelError::ConfigurationError { message } => {
                assert!(message.contains("Missing required field: value"));
            }
            _ => panic!("Expected ConfigurationError"),
        }

        // Test missing 'name' field error
        let result2 = TestConfigBuilder::new().with_value(42).build();

        assert!(result2.is_err());
        match result2.unwrap_err() {
            crate::error::FerrisWheelError::ConfigurationError { message } => {
                assert!(message.contains("Missing required field: name"));
            }
            _ => panic!("Expected ConfigurationError"),
        }
    }

    #[test]
    fn test_builder_default_trait() {
        #[derive(Debug, PartialEq)]
        pub struct TestConfig {
            name: String,
        }

        impl_builder! {
            TestConfigBuilder => TestConfig {
                name: String,
            }
        }

        // Test that Default trait is implemented
        let builder1 = TestConfigBuilder::default();
        let builder2 = TestConfigBuilder::new();

        // Both should create equivalent builders
        let result1 = builder1.with_name("test".to_string()).build();
        let result2 = builder2.with_name("test".to_string()).build();

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert_eq!(result1.unwrap().name, result2.unwrap().name);
    }
}
