//! Report generation modules for different output formats
//!
//! This module contains report generators for various output formats:
//! - human: Human-readable console output
//! - json: JSON format for programmatic use
//! - junit: JUnit XML format for CI/CD integration
//! - github: GitHub Actions format for PR comments

pub mod github;
pub mod human;
pub mod json;
pub mod junit;

use crate::detector::CycleDetector;
use crate::error::FerrisWheelError;

/// Common trait for all report generators
pub trait ReportGenerator {
    /// Generate a report from cycle detection results
    fn generate_report(&self, detector: &CycleDetector) -> Result<String, FerrisWheelError>;
}

// Re-export for convenience
pub use github::GitHubReportGenerator;
pub use human::HumanReportGenerator;
pub use json::JsonReportGenerator;
pub use junit::JunitReportGenerator;
