//! Integration tests for ferris-wheel using the library interface

use std::fs;
use std::io::Cursor;
use std::path::Path;

use ferris_wheel::analyzer::WorkspaceAnalyzer;
use ferris_wheel::detector::CycleDetector;
use ferris_wheel::graph::{DependencyGraphBuilder, GraphRenderer};
use tempfile::TempDir;

/// Type alias for crate definition: (name, normal_deps, dev_deps, build_deps)
type CrateDefinition<'a> = (&'a str, Vec<&'a str>, Vec<&'a str>, Vec<&'a str>);

/// Helper to create separate workspaces that will generate subgraphs
fn create_separate_workspaces(temp_dir: &TempDir) {
    let root = temp_dir.path();

    // Create core workspace
    create_workspace_with_crates(
        root,
        "core",
        vec![
            ("core-runtime", vec![], vec![], vec![]),
            ("core-storage", vec!["core-runtime"], vec![], vec![]),
            ("core-rpc", vec!["core-runtime"], vec![], vec![]),
        ],
    );

    // Create app workspace
    create_workspace_with_crates(
        root,
        "app",
        vec![
            (
                "app-backend",
                vec!["core-runtime", "core-storage", "core-rpc"],
                vec![],
                vec![],
            ),
            ("app-frontend", vec!["core-rpc"], vec![], vec![]),
            (
                "app-worker",
                vec!["core-storage", "app-backend"],
                vec![],
                vec![],
            ),
        ],
    );

    // Create tools workspace
    create_workspace_with_crates(
        root,
        "tools",
        vec![
            ("tools-cli", vec!["app-backend"], vec![], vec![]),
            ("tools-migrate", vec!["core-storage"], vec![], vec![]),
            ("tools-test-utils", vec![], vec![], vec![]),
        ],
    );

    // Add a cycle by making core-runtime dev-depend on app-backend
    let core_runtime_cargo = root.join("core/core-runtime/Cargo.toml");
    let mut cargo_content = fs::read_to_string(&core_runtime_cargo).unwrap();
    cargo_content
        .push_str("\n[dev-dependencies]\napp-backend = { path = \"../../app/app-backend\" }\n");
    fs::write(&core_runtime_cargo, cargo_content).unwrap();
}

fn create_workspace_with_crates(root: &Path, workspace_name: &str, crates: Vec<CrateDefinition>) {
    let workspace_dir = root.join(workspace_name);
    fs::create_dir_all(&workspace_dir).unwrap();

    // Create workspace Cargo.toml
    let members: Vec<String> = crates
        .iter()
        .map(|(name, _, _, _)| name.to_string())
        .collect();
    let workspace_toml = format!(
        r#"[workspace]
members = [{}]
resolver = "2"
"#,
        members
            .iter()
            .map(|m| format!("\"{m}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    fs::write(workspace_dir.join("Cargo.toml"), workspace_toml).unwrap();

    // Create each crate
    for (crate_name, normal_deps, dev_deps, build_deps) in crates {
        let crate_dir = workspace_dir.join(crate_name);
        fs::create_dir_all(&crate_dir).unwrap();

        let mut cargo_toml = format!(
            r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2021"
"#
        );

        if !normal_deps.is_empty() {
            cargo_toml.push_str("\n[dependencies]\n");
            for dep in normal_deps {
                // Handle cross-workspace dependencies
                let dep_path = if dep.starts_with("core-") {
                    format!("../../core/{dep}")
                } else if dep.starts_with("app-") {
                    format!("../../app/{dep}")
                } else if dep.starts_with("tools-") {
                    format!("../../tools/{dep}")
                } else {
                    format!("../{dep}")
                };
                cargo_toml.push_str(&format!("{dep} = {{ path = \"{dep_path}\" }}\n"));
            }
        }

        if !dev_deps.is_empty() {
            cargo_toml.push_str("\n[dev-dependencies]\n");
            for dep in dev_deps {
                let dep_path = if dep.starts_with("core-") {
                    format!("../../core/{dep}")
                } else if dep.starts_with("app-") {
                    format!("../../app/{dep}")
                } else if dep.starts_with("tools-") {
                    format!("../../tools/{dep}")
                } else {
                    format!("../{dep}")
                };
                cargo_toml.push_str(&format!("{dep} = {{ path = \"{dep_path}\" }}\n"));
            }
        }

        if !build_deps.is_empty() {
            cargo_toml.push_str("\n[build-dependencies]\n");
            for dep in build_deps {
                let dep_path = if dep.starts_with("core-") {
                    format!("../../core/{dep}")
                } else if dep.starts_with("app-") {
                    format!("../../app/{dep}")
                } else if dep.starts_with("tools-") {
                    format!("../../tools/{dep}")
                } else {
                    format!("../{dep}")
                };
                cargo_toml.push_str(&format!("{dep} = {{ path = \"{dep_path}\" }}\n"));
            }
        }

        fs::write(crate_dir.join("Cargo.toml"), cargo_toml).unwrap();

        // Create dummy lib.rs
        let src_dir = crate_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), "// Dummy lib file\n").unwrap();
    }
}

/// Test the example for README.md
#[test]
fn test_readme_example_generation() {
    let temp_dir = TempDir::new().unwrap();

    // Create 3 separate workspaces for a hypothetical monorepo
    create_separate_workspaces(&temp_dir);

    // Analyze the workspace using the library API
    let mut analyzer = WorkspaceAnalyzer::new();
    analyzer
        .discover_workspaces(&[temp_dir.path().to_path_buf()], None)
        .unwrap();

    // Build dependency graph
    let mut graph_builder = DependencyGraphBuilder::new(false, false, false);
    graph_builder
        .build_cross_workspace_graph(analyzer.workspaces(), analyzer.crate_to_workspace(), None)
        .unwrap();

    // Detect cycles
    let mut detector = CycleDetector::new();
    detector.detect_cycles(graph_builder.graph()).unwrap();
    let cycles = detector.cycles().to_vec();

    // Generate mermaid diagram
    let renderer = GraphRenderer::new(true, true);
    let mut output = Cursor::new(Vec::new());
    renderer
        .render_mermaid(graph_builder.graph(), &cycles, &mut output)
        .unwrap();

    let mermaid_output = String::from_utf8(output.into_inner()).unwrap();

    // Print the mermaid output for README
    println!("\n=== Mermaid Diagram for README.md ===\n");
    println!("{mermaid_output}");
    println!("\n=== End of Mermaid Diagram ===\n");

    // Verify it contains expected elements
    assert!(mermaid_output.contains("graph TD"));
    assert!(mermaid_output.contains("core"));
    assert!(mermaid_output.contains("app"));
    assert!(mermaid_output.contains("tools"));
    assert!(mermaid_output.contains("Legend"));
    assert!(mermaid_output.contains("CycleSeverity"));

    // Should detect the cycle
    assert!(!cycles.is_empty());
    assert!(mermaid_output.contains("Cycle"));
}
