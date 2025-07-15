//! JUnit XML format report generation

use std::fmt::Write;

use super::ReportGenerator;
use crate::detector::CycleDetector;
use crate::error::FerrisWheelError;

pub struct JunitReportGenerator;

impl Default for JunitReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl JunitReportGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ReportGenerator for JunitReportGenerator {
    fn generate_report(&self, detector: &CycleDetector) -> Result<String, FerrisWheelError> {
        let mut output = String::new();

        writeln!(output, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        writeln!(
            output,
            r#"<testsuites name="cargo-ferris-wheel" tests="1" failures="{}">"#,
            if detector.has_cycles() { "1" } else { "0" }
        )?;
        writeln!(
            output,
            r#"  <testsuite name="workspace-cycles" tests="1" failures="{}">"#,
            if detector.has_cycles() { "1" } else { "0" }
        )?;

        if detector.has_cycles() {
            writeln!(
                output,
                r#"    <testcase name="check-workspace-cycles" classname="ferris-wheel">"#
            )?;
            writeln!(
                output,
                r#"      <failure message="Workspace dependency cycles detected">"#
            )?;
            writeln!(
                output,
                "Found {} dependency cycles:",
                detector.cycle_count()
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
                writeln!(output, "\nCycle {}: {}", i + 1, workspace_names.join(" → "))?;

                let mut sorted_edges = cycle.edges().to_vec();
                sorted_edges.sort_by(|a, b| match a.from_crate().cmp(b.from_crate()) {
                    std::cmp::Ordering::Equal => a.to_crate().cmp(b.to_crate()),
                    other => other,
                });

                for edge in sorted_edges {
                    writeln!(
                        output,
                        "  {} → {} ({})",
                        edge.from_crate(),
                        edge.to_crate(),
                        edge.dependency_type()
                    )?;
                }
            }

            writeln!(output, r#"      </failure>"#)?;
            writeln!(output, r#"    </testcase>"#)?;
        } else {
            writeln!(
                output,
                r#"    <testcase name="check-workspace-cycles" classname="ferris-wheel" />"#
            )?;
        }

        writeln!(output, r#"  </testsuite>"#)?;
        writeln!(output, r#"</testsuites>"#)?;

        Ok(output)
    }
}
