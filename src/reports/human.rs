//! Human-readable console report generation

use std::fmt::Write;

use console::style;

use super::ReportGenerator;
use crate::detector::CycleDetector;
use crate::error::FerrisWheelError;
use crate::utils::string::pluralize;

pub struct HumanReportGenerator {
    max_cycles: Option<usize>,
}

impl HumanReportGenerator {
    pub fn new(max_cycles: Option<usize>) -> Self {
        Self { max_cycles }
    }
}

impl ReportGenerator for HumanReportGenerator {
    fn generate_report(&self, detector: &CycleDetector) -> Result<String, FerrisWheelError> {
        let mut output = String::new();

        if !detector.has_cycles() {
            write!(
                output,
                "\n{} No dependency cycles detected! Your workspaces have a clean dependency \
                 structure.\n",
                style("✅").green().bold()
            )?;
            return Ok(output);
        }

        write!(
            output,
            "\n{} Found {} dependency {}:\n\n",
            style("❌").red().bold(),
            style(detector.cycle_count()).red().bold(),
            pluralize("cycle", detector.cycle_count())
        )?;

        let cycles_to_show = match self.max_cycles {
            Some(limit) => detector
                .cycles()
                .iter()
                .take(limit)
                .enumerate()
                .collect::<Vec<_>>(),
            None => detector.cycles().iter().enumerate().collect::<Vec<_>>(),
        };

        let total_cycles = detector.cycle_count();
        let showing_all = self.max_cycles.is_none_or(|limit| limit >= total_cycles);

        for (i, cycle) in cycles_to_show {
            writeln!(output, "{} Cycle #{}", style("🔄").yellow(), i + 1)?;
            writeln!(output, "  {} Workspaces involved:", style("📦").blue())?;

            let mut workspace_names = cycle.workspace_names().to_vec();
            workspace_names.sort();
            for ws_name in workspace_names {
                writeln!(
                    output,
                    "    {} {}",
                    style("•").dim(),
                    style(&ws_name).bold()
                )?;
            }

            writeln!(
                output,
                "\n  {} Dependencies creating this cycle:",
                style("🔗").cyan()
            )?;

            // Group edges by direction
            let mut directions: Vec<_> = cycle.edges_by_direction().keys().collect();
            directions.sort();

            for (from_ws, to_ws) in directions {
                if let Some(edges) = cycle
                    .edges_by_direction()
                    .get(&(from_ws.clone(), to_ws.clone()))
                {
                    writeln!(
                        output,
                        "\n    {} {} → {}:",
                        style("📦").blue(),
                        style(from_ws).bold(),
                        style(to_ws).bold()
                    )?;
                    let mut sorted_edges = edges.clone();
                    sorted_edges.sort_by(|a, b| match a.from_crate().cmp(b.from_crate()) {
                        std::cmp::Ordering::Equal => a.to_crate().cmp(b.to_crate()),
                        other => other,
                    });
                    for edge in sorted_edges {
                        writeln!(
                            output,
                            "      {} {} → {} ({})",
                            style("→").dim(),
                            style(edge.from_crate()).yellow(),
                            style(edge.to_crate()).yellow(),
                            style(edge.dependency_type()).dim()
                        )?;
                    }
                }
            }
            writeln!(output)?;
        }

        if !showing_all {
            writeln!(
                output,
                "\n{} Showing {} of {} cycles. Use --max-cycles to see more.",
                style("ℹ️").blue(),
                style(
                    self.max_cycles
                        .expect("max_cycles must be Some when !showing_all")
                )
                .yellow(),
                style(total_cycles).yellow()
            )?;
        }

        writeln!(
            output,
            "\n{} To break these cycles, you need to remove at least one dependency from each \
             cycle.",
            style("💡").yellow()
        )?;
        writeln!(
            output,
            "{} Consider extracting shared code into a separate workspace that both can depend on.",
            style("💡").yellow()
        )?;
        writeln!(
            output,
            "{} Focus on the crates that appear in the most cycles for maximum impact.",
            style("💡").yellow()
        )?;

        Ok(output)
    }
}
