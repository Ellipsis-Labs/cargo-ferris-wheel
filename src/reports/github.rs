//! GitHub Actions format report generation

use std::fmt::Write;

use super::ReportGenerator;
use crate::detector::CycleDetector;
use crate::error::FerrisWheelError;

pub struct GitHubReportGenerator;

impl Default for GitHubReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubReportGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ReportGenerator for GitHubReportGenerator {
    fn generate_report(&self, detector: &CycleDetector) -> Result<String, FerrisWheelError> {
        let mut output = String::new();

        if !detector.has_cycles() {
            writeln!(
                output,
                "::notice title=Dependency Check::No workspace dependency cycles detected! ✅"
            )?;
            return Ok(output);
        }

        writeln!(
            output,
            "::error title=Dependency Cycles::Found {} workspace dependency cycle{}",
            detector.cycle_count(),
            if detector.cycle_count() == 1 { "" } else { "s" }
        )?;

        for (i, cycle) in detector.cycles().iter().enumerate() {
            writeln!(
                output,
                "::warning title=Cycle {}::Workspaces: {}",
                i + 1,
                cycle.workspace_names().join(" → ")
            )?;

            for edge in cycle.edges() {
                writeln!(
                    output,
                    "::notice::  {} → {} ({})",
                    edge.from_crate(),
                    edge.to_crate(),
                    edge.dependency_type()
                )?;
            }
        }

        writeln!(
            output,
            "::notice title=Recommendation::To break these cycles, consider extracting shared \
             code into a separate workspace that both can depend on."
        )?;

        Ok(output)
    }
}
