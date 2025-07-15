//! Executor for the affected command

use std::fmt::Write;

use miette::{Result, WrapErr};

use crate::analyzer::WorkspaceAnalyzer;
use crate::cli::OutputFormat;
use crate::commands::affected::{AffectedAnalysis, AffectedJsonReport};
use crate::config::AffectedConfig;
use crate::error::FerrisWheelError;
use crate::executors::CommandExecutor;
use crate::graph::DependencyGraphBuilder;
use crate::progress::ProgressReporter;

pub struct AffectedExecutor;

impl CommandExecutor for AffectedExecutor {
    type Config = AffectedConfig;

    fn execute(config: Self::Config) -> Result<()> {
        // Create progress reporter if we're in an interactive terminal
        let mut progress = if console::Term::stderr().is_term() {
            Some(ProgressReporter::new())
        } else {
            None
        };

        // Discover workspaces
        let mut analyzer = WorkspaceAnalyzer::new();
        analyzer
            .discover_workspaces(&config.paths, progress.as_mut())
            .wrap_err("Failed to discover workspaces")?;

        // Build dependency graph for analysis
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

        // Create affected analysis
        let filter = crate::dependency_filter::DependencyFilter::new(
            config.exclude_dev,
            config.exclude_build,
            config.exclude_target,
        );
        let affected_analysis = AffectedAnalysis::new(
            analyzer.workspaces(),
            analyzer.crate_to_workspace(),
            analyzer.crate_to_paths(),
            filter,
        )?;

        // Analyze affected files
        let result = affected_analysis.analyze_affected_files(&config.files);

        // Generate report based on format
        let report = match config.format {
            OutputFormat::Json => generate_json_report(&result, &affected_analysis, &config)?,
            OutputFormat::Human => generate_human_report(&result, &affected_analysis, &config)?,
            OutputFormat::GitHub => generate_github_report(&result, &config)?,
            OutputFormat::Junit => generate_junit_report(&result, &config)?,
        };

        println!("{report}");

        // Report unmatched files
        if !result.unmatched_files.is_empty() && config.format == OutputFormat::Human {
            eprintln!("\n⚠️  Warning: Could not map the following files to any crate:");
            for file in &result.unmatched_files {
                eprintln!("  - {file}");
            }
        }

        Ok(())
    }
}

fn generate_json_report(
    result: &crate::commands::affected::AffectedResult,
    analysis: &AffectedAnalysis,
    config: &AffectedConfig,
) -> Result<String, FerrisWheelError> {
    let report = if config.direct_only {
        // For direct_only mode, use the to_json_report method but filter to only
        // directly affected
        let full_report = result.to_json_report(analysis);
        AffectedJsonReport {
            affected_crates: full_report
                .affected_crates
                .into_iter()
                .filter(|crate_info| crate_info.is_directly_affected)
                .collect(),
            affected_workspaces: full_report
                .affected_workspaces
                .into_iter()
                .filter(|ws| result.directly_affected_workspaces.contains(&ws.name))
                .collect(),
            directly_affected_crates: result.directly_affected_crates.iter().cloned().collect(),
            directly_affected_workspaces: full_report.directly_affected_workspaces,
        }
    } else {
        result.to_json_report(analysis)
    };

    Ok(serde_json::to_string_pretty(&report)?)
}

fn generate_human_report(
    result: &crate::commands::affected::AffectedResult,
    analysis: &AffectedAnalysis,
    config: &AffectedConfig,
) -> Result<String, FerrisWheelError> {
    let mut output = String::new();

    writeln!(
        output,
        "\n📁 Analyzing {} changed files",
        config.files.len()
    )?;

    // Directly affected
    writeln!(output, "\n🎯 Directly affected:")?;
    if config.show_crates {
        writeln!(
            output,
            "  Crates: {}",
            result.directly_affected_crates.len()
        )?;
        let mut sorted_crates: Vec<_> = result.directly_affected_crates.iter().collect();
        sorted_crates.sort();
        for crate_name in sorted_crates {
            writeln!(output, "    - {crate_name}")?
        }
    }
    writeln!(
        output,
        "  Workspaces: {}",
        result.directly_affected_workspaces.len()
    )?;
    let mut sorted_workspaces: Vec<_> = result.directly_affected_workspaces.iter().collect();
    sorted_workspaces.sort();
    for ws_name in sorted_workspaces {
        writeln!(output, "    📦 {ws_name}")?;
        // Find and display the workspace path
        if let Some((path, _)) = analysis
            .workspaces()
            .iter()
            .find(|(_, ws_info)| ws_info.name() == ws_name)
        {
            writeln!(output, "      📍 Path: {}", path.display())?;
        }
    }

    // All affected (including reverse dependencies)
    if !config.direct_only {
        writeln!(
            output,
            "\n🔄 All affected (including reverse dependencies):"
        )?;
        if config.show_crates {
            writeln!(output, "  Crates: {}", result.all_affected_crates.len())?;
            let mut sorted_all_crates: Vec<_> = result.all_affected_crates.iter().collect();
            sorted_all_crates.sort();
            for crate_name in sorted_all_crates {
                if !result.directly_affected_crates.contains(crate_name) {
                    writeln!(output, "    - {crate_name} (indirect)")?
                }
            }
        }
        writeln!(
            output,
            "  Workspaces: {}",
            result.all_affected_workspaces.len()
        )?;
        let mut sorted_all_workspaces: Vec<_> = result.all_affected_workspaces.iter().collect();
        sorted_all_workspaces.sort();
        for ws_name in sorted_all_workspaces {
            if !result.directly_affected_workspaces.contains(ws_name) {
                writeln!(output, "    📦 {ws_name} (indirect)")?;
                // Find and display the workspace path
                if let Some((path, _)) = analysis
                    .workspaces()
                    .iter()
                    .find(|(_, ws_info)| ws_info.name() == ws_name)
                {
                    writeln!(output, "      📍 Path: {}", path.display())?;
                }
            }
        }
    }

    Ok(output)
}

fn generate_github_report(
    result: &crate::commands::affected::AffectedResult,
    config: &AffectedConfig,
) -> Result<String, FerrisWheelError> {
    let mut output = String::new();

    let workspaces = if config.direct_only {
        &result.directly_affected_workspaces
    } else {
        &result.all_affected_workspaces
    };

    writeln!(
        output,
        "::notice title=Affected Analysis::Analyzed {} files, found {} affected workspace{}",
        config.files.len(),
        workspaces.len(),
        if workspaces.len() == 1 { "" } else { "s" }
    )?;

    if !workspaces.is_empty() {
        let ws_list: Vec<_> = workspaces.iter().cloned().collect();
        writeln!(
            output,
            "::notice title=Affected Workspaces::{}",
            ws_list.join(", ")
        )?;
    }

    Ok(output)
}

fn generate_junit_report(
    result: &crate::commands::affected::AffectedResult,
    config: &AffectedConfig,
) -> Result<String, FerrisWheelError> {
    let mut output = String::new();

    writeln!(output, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        output,
        r#"<testsuites name="affected-analysis" tests="1" failures="0">"#
    )?;
    writeln!(
        output,
        r#"  <testsuite name="file-analysis" tests="1" failures="0">"#
    )?;
    writeln!(
        output,
        r#"    <testcase name="analyze-changed-files" classname="ferris-wheel">"#
    )?;

    writeln!(output, "      <system-out>")?;
    writeln!(output, "        Files analyzed: {}", config.files.len())?;
    writeln!(
        output,
        "        Directly affected crates: {}",
        result.directly_affected_crates.len()
    )?;
    writeln!(
        output,
        "        Directly affected workspaces: {}",
        result.directly_affected_workspaces.len()
    )?;

    if !config.direct_only {
        writeln!(
            output,
            "        All affected crates: {}",
            result.all_affected_crates.len()
        )?;
        writeln!(
            output,
            "        All affected workspaces: {}",
            result.all_affected_workspaces.len()
        )?;
    }

    writeln!(output, "      </system-out>")?;
    writeln!(output, r#"    </testcase>"#)?;
    writeln!(output, r#"  </testsuite>"#)?;
    writeln!(output, r#"</testsuites>"#)?;

    Ok(output)
}
