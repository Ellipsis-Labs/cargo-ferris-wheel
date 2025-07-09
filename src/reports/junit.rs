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

            for (i, cycle) in detector.cycles().iter().enumerate() {
                writeln!(
                    output,
                    "\nCycle {}: {}",
                    i + 1,
                    cycle.workspace_names().join(" → ")
                )?;
                for edge in cycle.edges() {
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
