//! Deps command executor

use console::style;
use miette::{IntoDiagnostic, Result, WrapErr};

use crate::analyzer::WorkspaceAnalyzer;
use crate::commands::deps::{WorkspaceDependencyAnalysis, WorkspaceDepsReportGenerator};
use crate::config::WorkspaceDepsConfig;
use crate::executors::CommandExecutor;
use crate::graph::DependencyGraphBuilder;
use crate::progress::ProgressReporter;

pub struct DepsExecutor;

impl CommandExecutor for DepsExecutor {
    type Config = WorkspaceDepsConfig;

    fn execute(config: Self::Config) -> Result<()> {
        eprintln!(
            "{} Analyzing workspace dependencies...\n",
            style("🔍").cyan()
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

        // Build dependency graph for workspace analysis
        let mut graph_builder = DependencyGraphBuilder::new(
            config.exclude_dev,
            config.exclude_build,
            config.exclude_target,
        );

        graph_builder
            .build_cross_workspace_graph(
                analyzer.workspaces(),
                analyzer.crate_to_workspace(),
                progress.as_ref(),
            )
            .wrap_err("Failed to build cross-workspace dependency graph")?;

        // Perform workspace dependency analysis
        let mut analysis = WorkspaceDependencyAnalysis::new(
            analyzer.workspaces(),
            analyzer.crate_to_workspace(),
            graph_builder.graph(),
        );

        // Generate report based on format and workspace filter
        let report_generator = WorkspaceDepsReportGenerator::new(
            config.workspace.as_deref(),
            config.reverse,
            config.transitive,
        );

        let report_result = match config.format {
            crate::cli::OutputFormat::Human => {
                report_generator.generate_human_report(&mut analysis)
            }
            crate::cli::OutputFormat::Json => report_generator.generate_json_report(&mut analysis),
            crate::cli::OutputFormat::Junit => {
                report_generator.generate_junit_report(&mut analysis)
            }
            crate::cli::OutputFormat::GitHub => {
                report_generator.generate_github_report(&mut analysis)
            }
        };

        match report_result {
            Ok(report) => println!("{report}"),
            Err(e) => {
                return Err(e)
                    .into_diagnostic()
                    .wrap_err("Failed to generate workspace dependency report");
            }
        }

        Ok(())
    }
}
