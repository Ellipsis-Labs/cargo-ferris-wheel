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

        let mut sorted_cycles: Vec<_> = detector.cycles().iter().collect();
        sorted_cycles.sort_by(|a, b| {
            let a_names = a.workspace_names();
            let b_names = b.workspace_names();
            let a_first = a_names.first().map(|s| s.as_str()).unwrap_or("");
            let b_first = b_names.first().map(|s| s.as_str()).unwrap_or("");
            a_first.cmp(b_first)
        });

        for (i, cycle) in sorted_cycles.iter().enumerate() {
            let mut workspace_names = cycle.workspace_names().to_vec();
            workspace_names.sort();
            writeln!(
                output,
                "::warning title=Cycle {}::Workspaces: {}",
                i + 1,
                workspace_names.join(" → ")
            )?;

            let mut sorted_edges = cycle.edges().to_vec();
            sorted_edges.sort_by(|a, b| match a.from_crate().cmp(b.from_crate()) {
                std::cmp::Ordering::Equal => a.to_crate().cmp(b.to_crate()),
                other => other,
            });

            for edge in sorted_edges {
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
