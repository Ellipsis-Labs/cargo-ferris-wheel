//! Configuration constants for ferris-wheel
//!
//! This module contains all configurable constants used throughout the
//! application. These values can be overridden through environment variables or
//! configuration files.

use std::time::Duration;

/// Progress bar configuration
pub mod progress {
    use super::*;

    /// Duration between progress bar updates
    pub const TICK_INTERVAL: Duration = Duration::from_millis(100);

    /// Spinner frames for the ferris wheel animation
    pub const SPINNER_FRAMES: &[&str] = &[
        "🎡 ", // Standard ferris wheel
        "🎡⊙", // With center dot
        "🎡◐", // Quarter filled
        "🎡◓", // Half filled
        "🎡◑", // Three quarters
        "🎡◒", // Another quarter
        "🎡○", // Empty circle
        "🎡●", // Full circle
    ];
}

/// Output formatting configuration
pub mod output {
    /// Default output format when not specified
    pub const DEFAULT_FORMAT: &str = "human";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_constants() {
        assert_eq!(progress::TICK_INTERVAL, Duration::from_millis(100));
        assert_eq!(progress::SPINNER_FRAMES.len(), 8);
    }

    #[test]
    fn test_output_constants() {
        assert_eq!(output::DEFAULT_FORMAT, "human");
    }
}
