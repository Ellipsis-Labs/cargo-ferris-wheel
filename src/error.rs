use std::path::PathBuf;

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("Invalid TOML syntax in '{file}'")]
#[diagnostic(
    code(ferris_wheel::toml_parse_error),
    help("Check the TOML syntax near the highlighted position")
)]
pub struct TomlParseError {
    pub file: String,
    #[source_code]
    pub source_code: NamedSource<String>,
    #[label("syntax error here")]
    pub span: Option<SourceSpan>,
    #[source]
    pub source: toml::de::Error,
}

#[derive(Error, Debug, Diagnostic)]
pub enum FerrisWheelError {
    #[error("Failed to read file '{path}'")]
    #[diagnostic(
        code(ferris_wheel::io_error),
        help("Check if the file exists and you have read permissions")
    )]
    FileReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    #[diagnostic(transparent)]
    TomlParseError(Box<TomlParseError>),

    #[error("JSON serialization error")]
    #[diagnostic(
        code(ferris_wheel::json_error),
        help("This is likely an internal error - please report it")
    )]
    Json(#[from] serde_json::Error),

    #[error("String formatting error")]
    #[diagnostic(
        code(ferris_wheel::fmt_error),
        help("This is likely an internal error - please report it")
    )]
    Fmt(#[from] std::fmt::Error),

    #[error("IO error")]
    #[diagnostic(
        code(ferris_wheel::io_error),
        help("Check file permissions and disk space")
    )]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {message}")]
    #[diagnostic(
        code(ferris_wheel::config_error),
        help("Check your command arguments and configuration")
    )]
    ConfigurationError { message: String },

    #[error("Graph error: {message}")]
    #[diagnostic(
        code(ferris_wheel::graph_error),
        help("This may be an internal error with graph processing")
    )]
    GraphError { message: String },
}

#[cfg(test)]
mod tests {
    use std::io;

    use miette::NamedSource;

    use super::*;

    #[test]
    fn test_toml_parse_error_display() {
        let source_code = "invalid = toml content";
        let toml_err = toml::from_str::<toml::Value>(source_code).unwrap_err();

        let error = TomlParseError {
            file: "test.toml".to_string(),
            source_code: NamedSource::new("test.toml", source_code.to_string()),
            span: Some((10, 4).into()),
            source: toml_err,
        };

        let error_str = error.to_string();
        assert_eq!(error_str, "Invalid TOML syntax in 'test.toml'");
    }

    #[test]
    fn test_file_read_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let error = FerrisWheelError::FileReadError {
            path: PathBuf::from("/tmp/missing.toml"),
            source: io_err,
        };

        let error_str = error.to_string();
        assert_eq!(error_str, "Failed to read file '/tmp/missing.toml'");
    }

    #[test]
    fn test_configuration_error() {
        let error = FerrisWheelError::ConfigurationError {
            message: "Invalid configuration value".to_string(),
        };

        let error_str = error.to_string();
        assert_eq!(
            error_str,
            "Configuration error: Invalid configuration value"
        );
    }

    #[test]
    fn test_graph_error() {
        let error = FerrisWheelError::GraphError {
            message: "Cycle detected in graph".to_string(),
        };

        let error_str = error.to_string();
        assert_eq!(error_str, "Graph error: Cycle detected in graph");
    }

    #[test]
    fn test_error_codes() {
        // Test that all error variants have proper diagnostic codes
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let file_err = FerrisWheelError::FileReadError {
            path: PathBuf::from("test.txt"),
            source: io_err,
        };

        // Verify the error has diagnostic information
        use miette::Diagnostic;
        assert!(file_err.code().is_some());
        assert!(file_err.help().is_some());
    }

    #[test]
    fn test_error_conversion_from_io() {
        let io_err = io::Error::other("some io error");
        let ferris_err: FerrisWheelError = io_err.into();

        match ferris_err {
            FerrisWheelError::Io(_) => {}
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_error_conversion_from_json() {
        let json_str = "{invalid json}";
        let json_err = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let ferris_err: FerrisWheelError = json_err.into();

        match ferris_err {
            FerrisWheelError::Json(_) => {}
            _ => panic!("Expected Json variant"),
        }
    }
}
