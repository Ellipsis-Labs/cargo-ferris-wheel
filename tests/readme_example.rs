//! Example generation for README.md

use std::io::Cursor;

use ferris_wheel::detector::{CycleDetector, WorkspaceCycle};
use ferris_wheel::graph::{DependencyEdge, DependencyType, GraphRenderer, WorkspaceNode};
use petgraph::graph::DiGraph;

#[test]
fn generate_readme_example() {
    // Create a graph representing a hypothetical cargo monorepo
    let mut graph = DiGraph::new();

    // Core workspace nodes
    let core_runtime = graph.add_node(WorkspaceNode {
        name: "core-runtime".to_string(),
        crates: vec!["runtime".to_string(), "runtime-types".to_string()],
    });

    let core_storage = graph.add_node(WorkspaceNode {
        name: "core-storage".to_string(),
        crates: vec!["storage".to_string(), "storage-api".to_string()],
    });

    let core_rpc = graph.add_node(WorkspaceNode {
        name: "core-rpc".to_string(),
        crates: vec!["rpc-server".to_string(), "rpc-client".to_string()],
    });

    // Application workspace nodes
    let app_backend = graph.add_node(WorkspaceNode {
        name: "app-backend".to_string(),
        crates: vec![
            "backend-api".to_string(),
            "backend-service".to_string(),
            "backend-db".to_string(),
        ],
    });

    let app_frontend = graph.add_node(WorkspaceNode {
        name: "app-frontend".to_string(),
        crates: vec!["frontend-ui".to_string(), "frontend-state".to_string()],
    });

    let app_worker = graph.add_node(WorkspaceNode {
        name: "app-worker".to_string(),
        crates: vec!["worker-jobs".to_string(), "worker-scheduler".to_string()],
    });

    // Tools workspace nodes
    let tools_cli = graph.add_node(WorkspaceNode {
        name: "tools-cli".to_string(),
        crates: vec!["cli".to_string()],
    });

    let tools_migrate = graph.add_node(WorkspaceNode {
        name: "tools-migrate".to_string(),
        crates: vec!["migrate".to_string()],
    });

    let tools_test_utils = graph.add_node(WorkspaceNode {
        name: "tools-test-utils".to_string(),
        crates: vec!["test-utils".to_string(), "test-fixtures".to_string()],
    });

    // Add core dependencies
    graph.add_edge(
        core_storage,
        core_runtime,
        DependencyEdge {
            from_crate: "storage".to_string(),
            to_crate: "runtime".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    graph.add_edge(
        core_rpc,
        core_runtime,
        DependencyEdge {
            from_crate: "rpc-server".to_string(),
            to_crate: "runtime".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    // Add app dependencies
    graph.add_edge(
        app_backend,
        core_runtime,
        DependencyEdge {
            from_crate: "backend-service".to_string(),
            to_crate: "runtime".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    graph.add_edge(
        app_backend,
        core_storage,
        DependencyEdge {
            from_crate: "backend-db".to_string(),
            to_crate: "storage".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    graph.add_edge(
        app_backend,
        core_rpc,
        DependencyEdge {
            from_crate: "backend-api".to_string(),
            to_crate: "rpc-server".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    graph.add_edge(
        app_frontend,
        core_rpc,
        DependencyEdge {
            from_crate: "frontend-ui".to_string(),
            to_crate: "rpc-client".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    graph.add_edge(
        app_worker,
        core_storage,
        DependencyEdge {
            from_crate: "worker-jobs".to_string(),
            to_crate: "storage".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    graph.add_edge(
        app_worker,
        app_backend,
        DependencyEdge {
            from_crate: "worker-scheduler".to_string(),
            to_crate: "backend-service".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    // Add tools dependencies
    graph.add_edge(
        tools_cli,
        app_backend,
        DependencyEdge {
            from_crate: "cli".to_string(),
            to_crate: "backend-api".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    graph.add_edge(
        tools_migrate,
        core_storage,
        DependencyEdge {
            from_crate: "migrate".to_string(),
            to_crate: "storage".to_string(),
            dependency_type: DependencyType::Normal,
            target: None,
        },
    );

    // Add dev dependencies for test utils
    graph.add_edge(
        app_backend,
        tools_test_utils,
        DependencyEdge {
            from_crate: "backend-service".to_string(),
            to_crate: "test-utils".to_string(),
            dependency_type: DependencyType::Dev,
            target: None,
        },
    );

    graph.add_edge(
        app_frontend,
        tools_test_utils,
        DependencyEdge {
            from_crate: "frontend-ui".to_string(),
            to_crate: "test-fixtures".to_string(),
            dependency_type: DependencyType::Dev,
            target: None,
        },
    );

    // Create a subtle cycle: core-runtime depends on app-backend in dev
    graph.add_edge(
        core_runtime,
        app_backend,
        DependencyEdge {
            from_crate: "runtime".to_string(),
            to_crate: "backend-service".to_string(),
            dependency_type: DependencyType::Dev,
            target: None,
        },
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
        .add_edge()
        .from_workspace("app-backend")
        .to_workspace("core-runtime")
        .from_crate("backend-service")
        .to_crate("runtime")
        .dependency_type("Normal")
        .build();

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
}
