//! Executor for the affected command

use std::fmt::Write;

use miette::{Result, WrapErr};
use serde::Serialize;

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
                analyzer.crate_path_to_workspace(),
                analyzer.crate_to_paths(),
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
            analyzer.crate_path_to_workspace(),
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
        let mut direct_crates: Vec<String> = result
            .directly_affected_crates
            .iter()
            .map(|crate_id| crate_id.name().to_string())
            .collect();
        direct_crates.sort();
        direct_crates.dedup();

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
            directly_affected_crates: direct_crates,
            directly_affected_workspaces: full_report.directly_affected_workspaces,
        }
    } else {
        result.to_json_report(analysis)
    };

    let report = AffectedCliJsonReport {
        report,
        unmatched_files: result.unmatched_files.clone(),
    };

    Ok(serde_json::to_string_pretty(&report)?)
}

#[derive(Serialize)]
struct AffectedCliJsonReport {
    #[serde(flatten)]
    report: AffectedJsonReport,
    unmatched_files: Vec<String>,
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
        let mut sorted_crates: Vec<_> = result
            .directly_affected_crates
            .iter()
            .map(|crate_id| {
                (
                    analysis
                        .workspace_name(crate_id)
                        .unwrap_or_else(|| "unknown".to_string()),
                    crate_id.name().to_string(),
                )
            })
            .collect();
        sorted_crates.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        for (workspace, crate_name) in sorted_crates {
            writeln!(output, "    - {crate_name} ({workspace})")?
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
            let mut sorted_all_crates: Vec<_> = result
                .all_affected_crates
                .iter()
                .filter(|crate_id| !result.directly_affected_crates.contains(*crate_id))
                .map(|crate_id| {
                    (
                        analysis
                            .workspace_name(crate_id)
                            .unwrap_or_else(|| "unknown".to_string()),
                        crate_id.name().to_string(),
                    )
                })
                .collect();
            sorted_all_crates.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            for (workspace, crate_name) in sorted_all_crates {
                writeln!(output, "    - {crate_name} ({workspace})")?;
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

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::generate_json_report;
    use crate::analyzer::WorkspaceAnalyzer;
    use crate::cli::OutputFormat;
    use crate::commands::affected::AffectedAnalysis;
    use crate::config::AffectedConfig;
    use crate::dependency_filter::DependencyFilter;

    fn create_test_workspace() -> TempDir {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/demo-app\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("crates/demo-app/src")).unwrap();
        fs::write(
            root.join("crates/demo-app/Cargo.toml"),
            "[package]\nname = \"demo-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(
            root.join("crates/demo-app/src/lib.rs"),
            "pub fn demo() {}\n",
        )
        .unwrap();
        fs::write(root.join("ci.yml"), "name: CI\n").unwrap();

        temp
    }

    fn create_workspace_with_reverse_dependent() -> TempDir {
        let temp = create_test_workspace();
        let root = temp.path();

        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/demo-app\", \"crates/demo-consumer\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("crates/demo-consumer/src")).unwrap();
        fs::write(
            root.join("crates/demo-consumer/Cargo.toml"),
            "[package]\nname = \"demo-consumer\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\ndemo-app = { path = \"../demo-app\" }\n",
        )
        .unwrap();
        fs::write(
            root.join("crates/demo-consumer/src/lib.rs"),
            "pub fn consume() { demo_app::demo(); }\n",
        )
        .unwrap();

        temp
    }

    fn generate_report(temp: &TempDir, files: Vec<String>, direct_only: bool) -> Value {
        let root = temp.path();
        let mut analyzer = WorkspaceAnalyzer::new();
        analyzer
            .discover_workspaces(&[root.to_path_buf()], None)
            .unwrap();
        let analysis = AffectedAnalysis::new(
            analyzer.workspaces(),
            analyzer.crate_path_to_workspace(),
            DependencyFilter::default(),
        )
        .unwrap();
        let config = AffectedConfig::builder()
            .with_files(files.clone())
            .with_paths(vec![root.to_path_buf()])
            .with_format(OutputFormat::Json)
            .with_direct_only(direct_only)
            .build()
            .unwrap();
        let result = analysis.analyze_affected_files(&files);
        let output = generate_json_report(&result, &analysis, &config).unwrap();

        serde_json::from_str(&output).unwrap()
    }

    #[test]
    fn json_report_includes_only_unmatched_file() {
        let temp = create_test_workspace();
        let unmatched_file = temp.path().join("ci.yml").display().to_string();
        let report = generate_report(&temp, vec![unmatched_file.clone()], false);

        assert_eq!(report["unmatched_files"], json!([unmatched_file]));
        assert_eq!(report["affected_crates"], json!([]));
        assert_eq!(report["affected_workspaces"], json!([]));
        assert_eq!(report["directly_affected_crates"], json!([]));
        assert_eq!(report["directly_affected_workspaces"], json!([]));
    }

    #[test]
    fn json_report_preserves_matched_and_unmatched_files() {
        let temp = create_test_workspace();
        let crate_file = temp
            .path()
            .join("crates/demo-app/src/lib.rs")
            .display()
            .to_string();
        let unmatched_file = temp.path().join("ci.yml").display().to_string();
        let report = generate_report(&temp, vec![crate_file, unmatched_file.clone()], false);

        assert_eq!(report["unmatched_files"], json!([unmatched_file]));
        assert_eq!(report["affected_crates"][0]["name"], "demo-app");
        assert_eq!(report["directly_affected_crates"], json!(["demo-app"]));
        assert_eq!(report["affected_workspaces"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn direct_only_json_report_preserves_unmatched_files() {
        let temp = create_workspace_with_reverse_dependent();
        let crate_file = temp
            .path()
            .join("crates/demo-app/src/lib.rs")
            .display()
            .to_string();
        let unmatched_file = temp.path().join("ci.yml").display().to_string();
        let files = vec![crate_file, unmatched_file.clone()];
        let normal_report = generate_report(&temp, files.clone(), false);
        let direct_only_report = generate_report(&temp, files, true);

        let normal_affected_crates: Vec<_> = normal_report["affected_crates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|crate_info| crate_info["name"].as_str().unwrap())
            .collect();
        let direct_only_affected_crates: Vec<_> = direct_only_report["affected_crates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|crate_info| crate_info["name"].as_str().unwrap())
            .collect();

        assert_eq!(normal_affected_crates, vec!["demo-app", "demo-consumer"]);
        assert_eq!(direct_only_affected_crates, vec!["demo-app"]);
        assert_eq!(normal_report["unmatched_files"], json!([unmatched_file]));
        assert_eq!(
            direct_only_report["unmatched_files"],
            json!([unmatched_file])
        );
    }

    #[test]
    fn json_report_includes_empty_unmatched_files_array() {
        let temp = create_test_workspace();
        let crate_file = temp
            .path()
            .join("crates/demo-app/src/lib.rs")
            .display()
            .to_string();
        let report = generate_report(&temp, vec![crate_file], false);

        assert_eq!(report["unmatched_files"], json!([]));
        assert!(report["unmatched_files"].is_array());
    }
}
