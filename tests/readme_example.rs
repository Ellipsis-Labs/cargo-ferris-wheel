//! Example generation for README.md

use std::io::Cursor;

use cargo_ferris_wheel::ConfigBuilder;
use cargo_ferris_wheel::detector::{CycleDetector, WorkspaceCycle};
use cargo_ferris_wheel::graph::{DependencyEdge, DependencyType, GraphRenderer, WorkspaceNode};
use petgraph::graph::DiGraph;

#[test]
fn generate_readme_example() -> miette::Result<()> {
    // Create a graph representing a hypothetical cargo monorepo
    let mut graph = DiGraph::new();

    // Core workspace nodes
    let core_runtime = graph.add_node(
        WorkspaceNode::builder()
            .with_name("core-runtime".to_string())
            .with_crates(vec!["runtime".to_string(), "runtime-types".to_string()])
            .build()
            .unwrap(),
    );

    let core_storage = graph.add_node(
        WorkspaceNode::builder()
            .with_name("core-storage".to_string())
            .with_crates(vec!["storage".to_string(), "storage-api".to_string()])
            .build()
            .unwrap(),
    );

    let core_rpc = graph.add_node(
        WorkspaceNode::builder()
            .with_name("core-rpc".to_string())
            .with_crates(vec!["rpc-server".to_string(), "rpc-client".to_string()])
            .build()
            .unwrap(),
    );

    // Application workspace nodes
    let app_backend = graph.add_node(
        WorkspaceNode::builder()
            .with_name("app-backend".to_string())
            .with_crates(vec![
                "backend-api".to_string(),
                "backend-service".to_string(),
                "backend-db".to_string(),
            ])
            .build()
            .unwrap(),
    );

    let app_frontend = graph.add_node(
        WorkspaceNode::builder()
            .with_name("app-frontend".to_string())
            .with_crates(vec![
                "frontend-ui".to_string(),
                "frontend-state".to_string(),
            ])
            .build()
            .unwrap(),
    );

    let app_worker = graph.add_node(
        WorkspaceNode::builder()
            .with_name("app-worker".to_string())
            .with_crates(vec![
                "worker-jobs".to_string(),
                "worker-scheduler".to_string(),
            ])
            .build()
            .unwrap(),
    );

    // Tools workspace nodes
    let tools_cli = graph.add_node(
        WorkspaceNode::builder()
            .with_name("tools-cli".to_string())
            .with_crates(vec!["cli".to_string()])
            .build()
            .unwrap(),
    );

    let tools_migrate = graph.add_node(
        WorkspaceNode::builder()
            .with_name("tools-migrate".to_string())
            .with_crates(vec!["migrate".to_string()])
            .build()
            .unwrap(),
    );

    let tools_test_utils = graph.add_node(
        WorkspaceNode::builder()
            .with_name("tools-test-utils".to_string())
            .with_crates(vec!["test-utils".to_string(), "test-fixtures".to_string()])
            .build()
            .unwrap(),
    );

    // Add core dependencies
    graph.add_edge(
        core_storage,
        core_runtime,
        DependencyEdge::builder()
            .with_from_crate("storage")
            .with_to_crate("runtime")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        core_rpc,
        core_runtime,
        DependencyEdge::builder()
            .with_from_crate("rpc-server")
            .with_to_crate("runtime")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Add app dependencies
    graph.add_edge(
        app_backend,
        core_runtime,
        DependencyEdge::builder()
            .with_from_crate("backend-service")
            .with_to_crate("runtime")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        app_backend,
        core_storage,
        DependencyEdge::builder()
            .with_from_crate("backend-db")
            .with_to_crate("storage")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        app_backend,
        core_rpc,
        DependencyEdge::builder()
            .with_from_crate("backend-api")
            .with_to_crate("rpc-server")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        app_frontend,
        core_rpc,
        DependencyEdge::builder()
            .with_from_crate("frontend-ui")
            .with_to_crate("rpc-client")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        app_worker,
        core_storage,
        DependencyEdge::builder()
            .with_from_crate("worker-jobs")
            .with_to_crate("storage")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        app_worker,
        app_backend,
        DependencyEdge::builder()
            .with_from_crate("worker-scheduler")
            .with_to_crate("backend-service")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Add tools dependencies
    graph.add_edge(
        tools_cli,
        app_backend,
        DependencyEdge::builder()
            .with_from_crate("cli")
            .with_to_crate("backend-api")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        tools_migrate,
        core_storage,
        DependencyEdge::builder()
            .with_from_crate("migrate")
            .with_to_crate("storage")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Add dev dependencies for test utils
    graph.add_edge(
        app_backend,
        tools_test_utils,
        DependencyEdge::builder()
            .with_from_crate("backend-service")
            .with_to_crate("test-utils")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        app_frontend,
        tools_test_utils,
        DependencyEdge::builder()
            .with_from_crate("frontend-ui")
            .with_to_crate("test-fixtures")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    // Create a subtle cycle: core-runtime depends on app-backend in dev
    graph.add_edge(
        core_runtime,
        app_backend,
        DependencyEdge::builder()
            .with_from_crate("runtime")
            .with_to_crate("backend-service")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    // Detect cycles
    let mut detector = CycleDetector::new();
    detector.detect_cycles(&graph).unwrap();
    let _cycles = detector.cycles().to_vec();

    // Create the cycle information manually since we know what it should be
    let cycle = WorkspaceCycle::builder()
        .add_edge()
        .from_workspace("core-runtime")
        .to_workspace("app-backend")
        .from_crate("runtime")
        .to_crate("backend-service")
        .dependency_type("Dev")
        .add_edge()?
        .from_workspace("app-backend")
        .to_workspace("core-runtime")
        .from_crate("backend-service")
        .to_crate("runtime")
        .dependency_type("Normal")
        .build()?;

    // Generate mermaid diagram
    let renderer = GraphRenderer::new(true, true);
    let mut output = Cursor::new(Vec::new());
    renderer
        .render_mermaid(&graph, &[cycle], &mut output)
        .unwrap();

    let mermaid_output = String::from_utf8(output.into_inner()).unwrap();

    // Print the mermaid output for README
    println!("\n=== Mermaid Diagram for README.md ===\n");
    println!("{mermaid_output}");
    println!("\n=== End of Mermaid Diagram ===\n");

    Ok(())
}
