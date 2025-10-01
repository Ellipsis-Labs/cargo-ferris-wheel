//! Graph command executor

use std::fs::File;
use std::io::{self, BufWriter};

use console::style;
use miette::{IntoDiagnostic, Result, WrapErr};

use crate::analyzer::WorkspaceAnalyzer;
use crate::cli::GraphFormat;
use crate::config::GraphOptions;
use crate::detector::CycleDetector;
use crate::executors::CommandExecutor;
use crate::graph::DependencyGraphBuilder;

pub struct GraphExecutor;

impl CommandExecutor for GraphExecutor {
    type Config = GraphOptions;

    fn execute(config: Self::Config) -> Result<()> {
        eprintln!(
            "{} Generating {} dependency graph...",
            style("📊").cyan(),
            format!("{:?}", config.format).to_lowercase()
        );

        // Discover and analyze workspaces
        let mut analyzer = WorkspaceAnalyzer::new();
        analyzer
            .discover_workspaces(&config.paths, None)
            .wrap_err("Failed to discover workspaces")?;

        if analyzer.workspaces().is_empty() {
            eprintln!("{} No workspaces found to visualize", style("ℹ").blue());
            return Ok(());
        }

        // Build dependency graph
        let mut graph_builder = DependencyGraphBuilder::new(
            config.exclude_dev,
            config.exclude_build,
            config.exclude_target,
        );
        graph_builder
            .build_cross_workspace_graph(
                analyzer.workspaces(),
                analyzer.crate_to_workspace(),
                analyzer.crate_path_to_workspace(),
                analyzer.crate_to_paths(),
                None,
            )
            .wrap_err("Failed to build dependency graph")?;

        // Detect cycles if highlighting is requested
        let cycles = if config.highlight_cycles {
            let mut detector = CycleDetector::new();
            detector
                .detect_cycles(graph_builder.graph())
                .wrap_err("Failed to detect cycles")?;
            detector.cycles().to_vec()
        } else {
            Vec::new()
        };

        // Create renderer
        let renderer =
            crate::graph::GraphRenderer::new(config.highlight_cycles, config.show_crates);

        // Determine output destination
        let mut output_writer: Box<dyn io::Write> =
            if let Some(output_path) = config.output.as_ref() {
                Box::new(BufWriter::new(
                    File::create(output_path)
                        .into_diagnostic()
                        .wrap_err_with(|| {
                            format!("Failed to create output file '{}'", output_path.display())
                        })?,
                ))
            } else {
                Box::new(io::stdout())
            };

        // Render based on format
        match config.format {
            GraphFormat::Ascii => {
                renderer
                    .render_ascii(graph_builder.graph(), &cycles, output_writer.as_mut())
                    .wrap_err("Failed to render ASCII graph")?;
            }
            GraphFormat::Mermaid => {
                renderer
                    .render_mermaid(graph_builder.graph(), &cycles, output_writer.as_mut())
                    .wrap_err("Failed to render Mermaid graph")?;
            }
            GraphFormat::Dot => {
                renderer
                    .render_dot(graph_builder.graph(), &cycles, output_writer.as_mut())
                    .wrap_err("Failed to render DOT graph")?;
            }
            GraphFormat::D2 => {
                renderer
                    .render_d2(graph_builder.graph(), &cycles, output_writer.as_mut())
                    .wrap_err("Failed to render D2 graph")?;
            }
        }

        if let Some(output_path) = config.output {
            eprintln!(
                "{} Graph written to {}",
                style("✓").green(),
                style(output_path.display()).bold()
            );
        }

        Ok(())
    }
}
