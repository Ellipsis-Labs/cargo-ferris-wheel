//! Check command executor

use console::style;
use miette::{IntoDiagnostic, Result, WrapErr};

use crate::analyzer::WorkspaceAnalyzer;
use crate::cli::OutputFormat;
use crate::config::CheckCyclesConfig;
use crate::detector::CycleDetector;
use crate::executors::CommandExecutor;
use crate::graph::DependencyGraphBuilder;
use crate::progress::ProgressReporter;
use crate::reports::{
    GitHubReportGenerator, HumanReportGenerator, JsonReportGenerator, JunitReportGenerator,
    ReportGenerator,
};

pub struct CheckExecutor;

impl CommandExecutor for CheckExecutor {
    type Config = CheckCyclesConfig;

    fn execute(config: Self::Config) -> Result<()> {
        if config.intra_workspace {
            eprintln!(
                "{} Checking for intra-workspace dependency cycles...\n",
                style("🎡").cyan()
            );
        } else {
            eprintln!(
                "{} Checking for inter-workspace dependency cycles...\n",
                style("🎡").cyan()
            );
        }

        // Create progress reporter if we're in an interactive terminal
        let mut progress = if console::Term::stderr().is_term() {
            Some(ProgressReporter::new())
        } else {
            None
        };

        // Discover and analyze workspaces
        let mut analyzer = WorkspaceAnalyzer::new();
        analyzer
            .discover_workspaces(&config.paths, progress.as_mut())
            .wrap_err("Failed to discover and analyze workspaces")?;

        if analyzer.workspaces().is_empty() {
            eprintln!("{} No workspaces found to analyze", style("ℹ").blue());
            return Ok(());
        }

        // Build dependency graph
        eprintln!("\n{} Building dependency graph...", style("🔨").blue());
        eprintln!(
            "  {} Exclude dev dependencies: {}",
            style("→").dim(),
            if config.exclude_dev {
                style("yes").red()
            } else {
                style("no").green()
            }
        );
        eprintln!(
            "  {} Exclude build dependencies: {}",
            style("→").dim(),
            if config.exclude_build {
                style("yes").red()
            } else {
                style("no").green()
            }
        );
        eprintln!(
            "  {} Exclude target dependencies: {}",
            style("→").dim(),
            if config.exclude_target {
                style("yes").red()
            } else {
                style("no").green()
            }
        );

        let mut graph_builder = DependencyGraphBuilder::new(
            config.exclude_dev,
            config.exclude_build,
            config.exclude_target,
        );

        if config.intra_workspace {
            graph_builder
                .build_intra_workspace_graph(analyzer.workspaces(), progress.as_ref())
                .wrap_err("Failed to build intra-workspace dependency graph")?;
        } else {
            graph_builder
                .build_cross_workspace_graph(
                    analyzer.workspaces(),
                    analyzer.crate_to_workspace(),
                    progress.as_ref(),
                )
                .wrap_err("Failed to build cross-workspace dependency graph")?;
        }

        // Detect cycles
        if let Some(p) = progress.as_mut() {
            p.start_cycle_detection();
        }

        let mut detector = CycleDetector::new();
        detector
            .detect_cycles(graph_builder.graph())
            .wrap_err("Failed to detect dependency cycles")?;

        if let Some(p) = progress.as_ref() {
            p.finish_cycle_detection(detector.cycle_count());
        }

        // Generate report based on format
        let report_result = match config.format {
            OutputFormat::Human => {
                let generator = HumanReportGenerator::new(config.max_cycles);
                generator.generate_report(&detector)
            }
            OutputFormat::Json => {
                let generator = JsonReportGenerator::new();
                generator.generate_report(&detector)
            }
            OutputFormat::Junit => {
                let generator = JunitReportGenerator::new();
                generator.generate_report(&detector)
            }
            OutputFormat::GitHub => {
                let generator = GitHubReportGenerator::new();
                generator.generate_report(&detector)
            }
        };

        match report_result {
            Ok(report) => print!("{report}"),
            Err(e) => {
                return Err(e)
                    .into_diagnostic()
                    .wrap_err("Failed to generate report");
            }
        }

        // Exit with error code if cycles found and requested
        if config.error_on_cycles && detector.has_cycles() {
            std::process::exit(1);
        }

        Ok(())
    }
}
