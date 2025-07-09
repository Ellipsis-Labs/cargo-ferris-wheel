//! Analyze command executor

use console::style;
use miette::{Result, WrapErr};

use crate::analyzer::WorkspaceAnalyzer;
use crate::cli::OutputFormat;
use crate::config::AnalyzeCrateConfig;
use crate::detector::CycleDetector;
use crate::executors::CommandExecutor;
use crate::graph::DependencyGraphBuilder;
use crate::progress::ProgressReporter;
use crate::reports::{
    GitHubReportGenerator, HumanReportGenerator, JsonReportGenerator, JunitReportGenerator,
    ReportGenerator,
};

pub struct AnalyzeExecutor;

impl CommandExecutor for AnalyzeExecutor {
    type Config = AnalyzeCrateConfig;

    fn execute(config: Self::Config) -> Result<()> {
        eprintln!(
            "{} Analyzing cycles involving crate '{}'...\n",
            style("🔍").cyan(),
            style(&config.crate_name).bold()
        );

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

        // Filter cycles that involve the specified crate
        let relevant_cycles: Vec<_> = detector
            .cycles()
            .iter()
            .filter(|cycle| {
                cycle.edges().iter().any(|edge| {
                    edge.from_crate().contains(&config.crate_name)
                        || edge.to_crate().contains(&config.crate_name)
                })
            })
            .cloned()
            .collect();

        if relevant_cycles.is_empty() {
            eprintln!(
                "{} No cycles found involving crate '{}'",
                style("✓").green(),
                style(&config.crate_name).bold()
            );
            return Ok(());
        }

        eprintln!(
            "\n{} Found {} cycle(s) involving '{}':",
            style("⚠").yellow(),
            relevant_cycles.len(),
            style(&config.crate_name).bold()
        );

        // Generate report based on format
        // For now, we'll create a custom detector with only the relevant cycles
        let mut filtered_detector = CycleDetector::new();
        for cycle in relevant_cycles {
            filtered_detector.add_cycle(cycle);
        }

        let report_result = match config.format {
            OutputFormat::Human => {
                let generator = HumanReportGenerator::new(config.max_cycles);
                generator.generate_report(&filtered_detector)
            }
            OutputFormat::Json => {
                let generator = JsonReportGenerator::new();
                generator.generate_report(&filtered_detector)
            }
            OutputFormat::Junit => {
                let generator = JunitReportGenerator::new();
                generator.generate_report(&filtered_detector)
            }
            OutputFormat::GitHub => {
                let generator = GitHubReportGenerator::new();
                generator.generate_report(&filtered_detector)
            }
        };

        match report_result {
            Ok(report) => print!("{report}"),
            Err(e) => {
                return Err(e).wrap_err("Failed to generate report for crate analysis");
            }
        }

        Ok(())
    }
}
