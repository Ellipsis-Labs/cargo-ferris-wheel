//! Tests for the graph renderer module

use std::io::Cursor;

use cargo_ferris_wheel::common::ConfigBuilder;
use cargo_ferris_wheel::detector::WorkspaceCycle;
use cargo_ferris_wheel::graph::{DependencyEdge, DependencyType, GraphRenderer, WorkspaceNode};
use petgraph::graph::DiGraph;

/// Create a test graph with duplicate edges between workspaces
fn create_test_graph_with_duplicates() -> DiGraph<WorkspaceNode, DependencyEdge> {
    let mut graph = DiGraph::new();

    // Add workspace nodes
    let nodes_ws = graph.add_node(
        WorkspaceNode::builder()
            .with_name("nodes".to_string())
            .with_crates(vec![
                "sequencer-node".to_string(),
                "replay-node".to_string(),
                "phoenix-node".to_string(),
                "test-validator".to_string(),
            ])
            .build()
            .unwrap(),
    );

    let core_ws = graph.add_node(
        WorkspaceNode::builder()
            .with_name("core".to_string())
            .with_crates(vec![
                "atlas-core".to_string(),
                "atlas-scheduler".to_string(),
                "atlas-storage".to_string(),
            ])
            .build()
            .unwrap(),
    );

    let tools_ws = graph.add_node(
        WorkspaceNode::builder()
            .with_name("tools".to_string())
            .with_crates(vec!["ferris-wheel".to_string(), "atlas-cli".to_string()])
            .build()
            .unwrap(),
    );

    // Add multiple edges from nodes to core (simulating multiple crate
    // dependencies)
    graph.add_edge(
        nodes_ws,
        core_ws,
        DependencyEdge::builder()
            .with_from_crate("sequencer-node")
            .with_to_crate("atlas-core")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        nodes_ws,
        core_ws,
        DependencyEdge::builder()
            .with_from_crate("replay-node")
            .with_to_crate("atlas-core")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        nodes_ws,
        core_ws,
        DependencyEdge::builder()
            .with_from_crate("phoenix-node")
            .with_to_crate("atlas-scheduler")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        nodes_ws,
        core_ws,
        DependencyEdge::builder()
            .with_from_crate("test-validator")
            .with_to_crate("atlas-storage")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Add some dev dependencies
    graph.add_edge(
        nodes_ws,
        core_ws,
        DependencyEdge::builder()
            .with_from_crate("sequencer-node")
            .with_to_crate("atlas-scheduler")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    // Add edge from tools to core
    graph.add_edge(
        tools_ws,
        core_ws,
        DependencyEdge::builder()
            .with_from_crate("ferris-wheel")
            .with_to_crate("atlas-core")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph
}

#[test]
fn test_mermaid_duplicate_edges_without_crates() {
    let graph = create_test_graph_with_duplicates();
    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();
    println!("Mermaid output without crates:\n{result}");

    // Verify it doesn't have markdown backticks
    assert!(
        !result.contains("```"),
        "Should not contain markdown backticks"
    );
    assert!(
        result.starts_with("graph TD"),
        "Should start with mermaid graph directive"
    );

    // Verify aggregation: should show "📦 4 uses" instead of 4 duplicate edges
    assert!(
        result.contains("nodes -->|📦 4 uses| core"),
        "Should aggregate 4 Normal edges into one with count"
    );

    // Count how many times any edge from nodes to core appears (different arrow
    // types now)
    let edge_lines: Vec<&str> = result
        .lines()
        .filter(|line| {
            line.contains("nodes")
                && line.contains("| core")
                && (line.contains("-->|") || line.contains("-.->|"))
        })
        .collect();

    // Should have exactly 2 edges: one aggregated Normal and one Dev
    assert_eq!(
        edge_lines.len(),
        2,
        "Should have exactly 2 edges from nodes to core (Normal and Dev)"
    );

    // Also check for dev dependency edge with new arrow type
    assert!(result.contains("nodes -.->|🔧 dev| core"));

    // And the single edge from tools
    assert!(result.contains("tools -->|📦 uses| core"));
}

#[test]
fn test_mermaid_duplicate_edges_with_crates() {
    let graph = create_test_graph_with_duplicates();
    let renderer = GraphRenderer::new(false, true);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();
    println!("Mermaid output with crates:\n{result}");

    // With crates shown, Normal deps are still aggregated but show count with icon
    assert!(result.contains("nodes -->|📦 4 (normal)| core"));
    // Dev dependency should show individual crate names
    assert!(result.contains("sequencer-node → atlas-scheduler"));
}

#[test]
fn test_ascii_duplicate_edges() {
    let graph = create_test_graph_with_duplicates();
    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_ascii(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();
    println!("ASCII output:\n{result}");

    // Check that nodes workspace shows core as dependency
    assert!(result.contains("nodes"));
    assert!(result.contains("→ core"));
}

#[test]
fn test_cycle_summary() -> miette::Result<()> {
    let mut graph = DiGraph::new();

    // Create a cycle between three workspaces
    let ws_a = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-a".to_string())
            .with_crates(vec!["crate-a1".to_string(), "crate-a2".to_string()])
            .build()
            .unwrap(),
    );

    let ws_b = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-b".to_string())
            .with_crates(vec!["crate-b".to_string()])
            .build()
            .unwrap(),
    );

    let ws_c = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-c".to_string())
            .with_crates(vec!["crate-c".to_string()])
            .build()
            .unwrap(),
    );

    // Add edges to form a cycle A -> B -> C -> A
    graph.add_edge(
        ws_a,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("crate-a1")
            .with_to_crate("crate-b")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_b,
        ws_c,
        DependencyEdge::builder()
            .with_from_crate("crate-b")
            .with_to_crate("crate-c")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Use Dev dependency to close the cycle (easier to break)
    graph.add_edge(
        ws_c,
        ws_a,
        DependencyEdge::builder()
            .with_from_crate("crate-c")
            .with_to_crate("crate-a1")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    // Create a cycle with edges
    let cycle = WorkspaceCycle::builder()
        .add_edge()
        .from_workspace("workspace-a")
        .to_workspace("workspace-b")
        .from_crate("crate-a1")
        .to_crate("crate-b")
        .dependency_type("Normal")
        .add_edge()?
        .from_workspace("workspace-b")
        .to_workspace("workspace-c")
        .from_crate("crate-b")
        .to_crate("crate-c")
        .dependency_type("Normal")
        .add_edge()?
        .from_workspace("workspace-c")
        .to_workspace("workspace-a")
        .from_crate("crate-c")
        .to_crate("crate-a1")
        .dependency_type("Dev")
        .build()?;

    let cycles = vec![cycle];

    let renderer = GraphRenderer::new(true, true);
    let mut output = Cursor::new(Vec::new());

    renderer.render_cycle_summary(&cycles, &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();
    println!("Cycle summary output:\n{result}");

    // Verify key information is present
    assert!(result.contains("Dependency Cycles Summary"));
    assert!(result.contains("workspace-a → workspace-b → workspace-c"));
    assert!(result.contains("Total edges in cycle: 3"));
    assert!(result.contains("Suggested break points"));
    assert!(result.contains("workspace-c → workspace-a (1 dev/build dependencies)"));

    Ok(())
}

#[test]
fn test_edge_highlighting_with_cycles() {
    let mut graph = DiGraph::new();

    // Create a triangle of workspaces with a cycle
    let ws_a = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-a".to_string())
            .with_crates(vec!["crate-a".to_string()])
            .build()
            .unwrap(),
    );

    let ws_b = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-b".to_string())
            .with_crates(vec!["crate-b".to_string()])
            .build()
            .unwrap(),
    );

    let ws_c = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-c".to_string())
            .with_crates(vec!["crate-c".to_string()])
            .build()
            .unwrap(),
    );

    // Create edges: A -> B, B -> C, C -> A (cycle)
    graph.add_edge(
        ws_a,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("crate-a")
            .with_to_crate("crate-b")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_b,
        ws_c,
        DependencyEdge::builder()
            .with_from_crate("crate-b")
            .with_to_crate("crate-c")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_c,
        ws_a,
        DependencyEdge::builder()
            .with_from_crate("crate-c")
            .with_to_crate("crate-a")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Add an extra edge between cycle members (A -> C) to test if it's also
    // highlighted
    graph.add_edge(
        ws_a,
        ws_c,
        DependencyEdge::builder()
            .with_from_crate("crate-a")
            .with_to_crate("crate-c")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    // Create a cycle for the test
    let cycle = WorkspaceCycle::builder()
        .with_workspace_names(vec![
            "workspace-a".to_string(),
            "workspace-b".to_string(),
            "workspace-c".to_string(),
        ])
        .build();

    let cycles = vec![cycle];
    let renderer = GraphRenderer::new(true, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_ascii(&graph, &cycles, &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();
    println!("ASCII output with improved edge highlighting:\n{result}");

    // Verify all edges between cycle members are highlighted
    assert!(result.contains("→ workspace-b (normal) ⚠️  [CYCLE]"));
    assert!(result.contains("→ workspace-c (normal) ⚠️  [CYCLE]"));
    assert!(result.contains("→ workspace-a (normal) ⚠️  [CYCLE]"));
    assert!(result.contains("→ workspace-c (dev) ⚠️  [CYCLE]")); // The extra edge should also be highlighted
}

#[test]
fn test_graph_with_cycles() -> miette::Result<()> {
    let mut graph = DiGraph::new();

    // Create a cycle between two workspaces
    let ws_a = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-a".to_string())
            .with_crates(vec!["crate-a".to_string()])
            .build()
            .unwrap(),
    );

    let ws_b = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-b".to_string())
            .with_crates(vec!["crate-b".to_string()])
            .build()
            .unwrap(),
    );

    // A depends on B
    graph.add_edge(
        ws_a,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("crate-a")
            .with_to_crate("crate-b")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // B depends on A (creating a cycle)
    graph.add_edge(
        ws_b,
        ws_a,
        DependencyEdge::builder()
            .with_from_crate("crate-b")
            .with_to_crate("crate-a")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    let cycles = vec![
        WorkspaceCycle::builder()
            .add_edge()
            .from_workspace("workspace-a")
            .to_workspace("workspace-b")
            .from_crate("crate-a")
            .to_crate("crate-b")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("workspace-b")
            .to_workspace("workspace-a")
            .from_crate("crate-b")
            .to_crate("crate-a")
            .dependency_type("Normal")
            .build()?,
    ];

    let renderer = GraphRenderer::new(true, false);
    let mut output = Cursor::new(Vec::new());

    renderer
        .render_mermaid(&graph, &cycles, &mut output)
        .unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Check that cycle highlighting is present with the new Blue-Orange palette
    assert!(result.contains("fill:#FFF3E0")); // Light orange fill for nodes in cycle
    assert!(result.contains("stroke:#FF6500")); // Deep orange stroke for cycle edges

    Ok(())
}

#[test]
fn test_dot_format_duplicate_edges() {
    let graph = create_test_graph_with_duplicates();
    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_dot(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();
    println!("DOT output:\n{result}");

    // Verify aggregation in DOT format
    assert!(
        result.contains(
            r##""nodes" -> "core" [label="Normal - 4 deps", color="#64B5F6", penwidth=2]"##
        ),
        "Should have aggregated Normal edges from nodes to core"
    );
}

#[test]
fn test_d2_format_duplicate_edges() {
    let graph = create_test_graph_with_duplicates();
    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_d2(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();
    println!("D2 output:\n{result}");

    // Check D2 format
    assert!(result.contains("nodes -> core: Normal"));
}

#[test]
fn test_mermaid_empty_graph() {
    let graph = DiGraph::new();
    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Empty graph should still have the graph directive
    assert!(result.contains("graph TD"));
}

#[test]
fn test_mermaid_single_workspace_no_dependencies() {
    let mut graph = DiGraph::new();

    graph.add_node(
        WorkspaceNode::builder()
            .with_name("standalone".to_string())
            .with_crates(vec!["crate1".to_string(), "crate2".to_string()])
            .build()
            .unwrap(),
    );

    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Should contain the node
    assert!(result.contains("standalone[\"standalone\"]"));
    // Should have normal styling with new colors
    assert!(result.contains("style standalone fill:#E3F2FD,stroke:#1976D2"));
}

#[test]
fn test_mermaid_all_dependency_types() {
    let mut graph = DiGraph::new();

    let ws_a = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-a".to_string())
            .with_crates(vec!["crate-a".to_string()])
            .build()
            .unwrap(),
    );

    let ws_b = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-b".to_string())
            .with_crates(vec!["crate-b".to_string()])
            .build()
            .unwrap(),
    );

    let ws_c = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-c".to_string())
            .with_crates(vec!["crate-c".to_string()])
            .build()
            .unwrap(),
    );

    // Add different types of dependencies
    graph.add_edge(
        ws_a,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("crate-a")
            .with_to_crate("crate-b")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_a,
        ws_c,
        DependencyEdge::builder()
            .with_from_crate("crate-a")
            .with_to_crate("crate-c")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_b,
        ws_c,
        DependencyEdge::builder()
            .with_from_crate("crate-b")
            .with_to_crate("crate-c")
            .with_dependency_type(DependencyType::Build)
            .build()
            .unwrap(),
    );

    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Check that all dependency types are represented with new icons and arrow
    // types
    assert!(result.contains(r#"workspace_a -->|📦 uses| workspace_b"#));
    assert!(result.contains(r#"workspace_a -.->|🔧 dev| workspace_c"#));
    assert!(result.contains(r#"workspace_b ===>|🏗️ build| workspace_c"#));

    // Check that different edge colors are applied
    assert!(result.contains("stroke:#64B5F6")); // Normal edge color
    assert!(result.contains("stroke:#90A4AE")); // Dev edge color
    assert!(result.contains("stroke:#81C784")); // Build edge color
}

#[test]
fn test_mermaid_special_characters_in_names() {
    let mut graph = DiGraph::new();

    let ws_special = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-with-dashes".to_string())
            .with_crates(vec!["my-special-crate".to_string()])
            .build()
            .unwrap(),
    );

    let ws_spaces = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace with spaces".to_string())
            .with_crates(vec!["crate with spaces".to_string()])
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_special,
        ws_spaces,
        DependencyEdge::builder()
            .with_from_crate("my-special-crate")
            .with_to_crate("crate with spaces")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    let renderer = GraphRenderer::new(false, true);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Check that special characters are handled (replaced with underscores)
    assert!(result.contains("workspace_with_dashes"));
    assert!(result.contains("workspace_with_spaces"));
}

#[test]
fn test_mermaid_complex_multi_cycle() -> miette::Result<()> {
    let mut graph = DiGraph::new();

    // Create a complex graph with multiple cycles
    // A -> B -> C -> A (cycle 1)
    // B -> D -> E -> B (cycle 2)

    let ws_a = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-a".to_string())
            .with_crates(vec!["crate-a".to_string()])
            .build()
            .unwrap(),
    );

    let ws_b = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-b".to_string())
            .with_crates(vec!["crate-b".to_string()])
            .build()
            .unwrap(),
    );

    let ws_c = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-c".to_string())
            .with_crates(vec!["crate-c".to_string()])
            .build()
            .unwrap(),
    );

    let ws_d = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-d".to_string())
            .with_crates(vec!["crate-d".to_string()])
            .build()
            .unwrap(),
    );

    let ws_e = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-e".to_string())
            .with_crates(vec!["crate-e".to_string()])
            .build()
            .unwrap(),
    );

    // Cycle 1: A -> B -> C -> A
    graph.add_edge(
        ws_a,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("crate-a")
            .with_to_crate("crate-b")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_b,
        ws_c,
        DependencyEdge::builder()
            .with_from_crate("crate-b")
            .with_to_crate("crate-c")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_c,
        ws_a,
        DependencyEdge::builder()
            .with_from_crate("crate-c")
            .with_to_crate("crate-a")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Cycle 2: B -> D -> E -> B
    graph.add_edge(
        ws_b,
        ws_d,
        DependencyEdge::builder()
            .with_from_crate("crate-b")
            .with_to_crate("crate-d")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_d,
        ws_e,
        DependencyEdge::builder()
            .with_from_crate("crate-d")
            .with_to_crate("crate-e")
            .with_dependency_type(DependencyType::Build)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_e,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("crate-e")
            .with_to_crate("crate-b")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Create cycle information for rendering
    let cycles = vec![
        WorkspaceCycle::builder()
            .add_edge()
            .from_workspace("workspace-a")
            .to_workspace("workspace-b")
            .from_crate("crate-a")
            .to_crate("crate-b")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("workspace-b")
            .to_workspace("workspace-c")
            .from_crate("crate-b")
            .to_crate("crate-c")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("workspace-c")
            .to_workspace("workspace-a")
            .from_crate("crate-c")
            .to_crate("crate-a")
            .dependency_type("Normal")
            .build()?,
        WorkspaceCycle::builder()
            .add_edge()
            .from_workspace("workspace-b")
            .to_workspace("workspace-d")
            .from_crate("crate-b")
            .to_crate("crate-d")
            .dependency_type("Dev")
            .add_edge()?
            .from_workspace("workspace-d")
            .to_workspace("workspace-e")
            .from_crate("crate-d")
            .to_crate("crate-e")
            .dependency_type("Build")
            .add_edge()?
            .from_workspace("workspace-e")
            .to_workspace("workspace-b")
            .from_crate("crate-e")
            .to_crate("crate-b")
            .dependency_type("Normal")
            .build()?,
    ];

    let renderer = GraphRenderer::new(true, false);
    let mut output = Cursor::new(Vec::new());

    renderer
        .render_mermaid(&graph, &cycles, &mut output)
        .unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Check that workspace B is highlighted (part of both cycles)
    assert!(result.contains("style workspace_b fill:#FFF3E0"));

    // Check legend is present
    assert!(result.contains("subgraph Legend"));
    assert!(result.contains("L1[Normal Workspace]"));
    assert!(result.contains("L2[Workspace in Cycle]"));

    Ok(())
}

#[test]
fn test_mermaid_with_show_crates() {
    let mut graph = DiGraph::new();

    let ws_a = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-a".to_string())
            .with_crates(vec![
                "crate-a1".to_string(),
                "crate-a2".to_string(),
                "crate-a3".to_string(),
            ])
            .build()
            .unwrap(),
    );

    let ws_b = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-b".to_string())
            .with_crates(vec!["crate-b1".to_string()])
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_a,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("crate-a1")
            .with_to_crate("crate-b1")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    let renderer = GraphRenderer::new(false, true);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Should show crate counts in node labels
    assert!(result.contains("workspace-a\\n3 crates"));
    assert!(result.contains("workspace-b\\n1 crates"));

    // Should show individual crate dependency
    assert!(result.contains("crate-a1 → crate-b1"));
}

#[test]
fn test_mermaid_with_subgraphs() {
    let mut graph = DiGraph::new();

    // Create workspaces with common prefixes
    let atlas_core = graph.add_node(
        WorkspaceNode::builder()
            .with_name("atlas-core".to_string())
            .with_crates(vec!["core1".to_string(), "core2".to_string()])
            .build()
            .unwrap(),
    );

    let atlas_storage = graph.add_node(
        WorkspaceNode::builder()
            .with_name("atlas-storage".to_string())
            .with_crates(vec!["storage1".to_string()])
            .build()
            .unwrap(),
    );

    let atlas_network = graph.add_node(
        WorkspaceNode::builder()
            .with_name("atlas-network".to_string())
            .with_crates(vec![
                "net1".to_string(),
                "net2".to_string(),
                "net3".to_string(),
            ])
            .build()
            .unwrap(),
    );

    let other_tool = graph.add_node(
        WorkspaceNode::builder()
            .with_name("other-tool".to_string())
            .with_crates(vec!["tool1".to_string()])
            .build()
            .unwrap(),
    );

    // Add some dependencies
    graph.add_edge(
        atlas_network,
        atlas_core,
        DependencyEdge::builder()
            .with_from_crate("net1")
            .with_to_crate("core1")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        atlas_storage,
        atlas_core,
        DependencyEdge::builder()
            .with_from_crate("storage1")
            .with_to_crate("core2")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        other_tool,
        atlas_core,
        DependencyEdge::builder()
            .with_from_crate("tool1")
            .with_to_crate("core1")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Check that atlas workspaces are grouped
    assert!(result.contains("subgraph atlas_group[\"atlas\"*]"));

    // Check that tooltips are present
    assert!(result.contains("click atlas_core"));
    assert!(result.contains("Workspace: atlas-core - Crates: core1, core2 - Total: 2"));

    // Check that other-tool is not in a subgraph (only one with "other" prefix)
    assert!(!result.contains("subgraph other_group"));
}

#[test]
fn test_mermaid_tooltips() {
    let mut graph = DiGraph::new();

    let ws_many_crates = graph.add_node(
        WorkspaceNode::builder()
            .with_name("many-crates".to_string())
            .with_crates(vec![
                "crate1".to_string(),
                "crate2".to_string(),
                "crate3".to_string(),
                "crate4".to_string(),
                "crate5".to_string(),
            ])
            .build()
            .unwrap(),
    );

    let ws_few_crates = graph.add_node(
        WorkspaceNode::builder()
            .with_name("few-crates".to_string())
            .with_crates(vec!["single".to_string()])
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_many_crates,
        ws_few_crates,
        DependencyEdge::builder()
            .with_from_crate("crate1")
            .with_to_crate("single")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Check tooltip for workspace with many crates (should truncate)
    assert!(result.contains("click many_crates"));
    assert!(result.contains(
        "Workspace: many-crates - Crates: crate1, crate2, crate3, ... (5 total) - Total: 5"
    ));

    // Check tooltip for workspace with few crates (should list all)
    assert!(result.contains("click few_crates"));
    assert!(result.contains("Workspace: few-crates - Crates: single - Total: 1"));
}

#[test]
fn test_mermaid_large_graph_performance() {
    let mut graph = DiGraph::new();

    // Create a large graph with 20 workspaces
    let mut nodes = Vec::new();
    for i in 0..20 {
        let node = graph.add_node(
            WorkspaceNode::builder()
                .with_name(format!("workspace-{i}"))
                .with_crates(vec![format!("crate-{}-1", i), format!("crate-{}-2", i)])
                .build()
                .unwrap(),
        );
        nodes.push(node);
    }

    // Add dependencies in a complex pattern
    for i in 0..19 {
        for j in (i + 1)..20 {
            if (j - i) <= 3 {
                // Connect nodes that are close
                graph.add_edge(
                    nodes[i],
                    nodes[j],
                    DependencyEdge::builder()
                        .with_from_crate(&format!("crate-{i}-1"))
                        .with_to_crate(&format!("crate-{j}-1"))
                        .with_dependency_type(match j - i {
                            1 => DependencyType::Normal,
                            2 => DependencyType::Dev,
                            _ => DependencyType::Build,
                        })
                        .build()
                        .unwrap(),
                );
            }
        }
    }

    let renderer = GraphRenderer::new(false, false);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Basic sanity checks
    assert!(result.contains("graph TD"));
    assert!(result.contains("workspace_0[\"workspace-0\"]"));
    assert!(result.contains("workspace_19[\"workspace-19\"]"));

    // Check that multiple edge types are present with new labels
    assert!(result.contains("📦 uses"));
    assert!(result.contains("🔧 dev"));
    assert!(result.contains("🏗️ build"));
}

#[test]
fn test_mermaid_target_specific_dependencies() {
    let mut graph = DiGraph::new();

    let ws_a = graph.add_node(
        WorkspaceNode::builder()
            .with_name("cross-platform".to_string())
            .with_crates(vec!["my-crate".to_string()])
            .build()
            .unwrap(),
    );

    let ws_b = graph.add_node(
        WorkspaceNode::builder()
            .with_name("platform-specific".to_string())
            .with_crates(vec!["platform-crate".to_string()])
            .build()
            .unwrap(),
    );

    // Add target-specific dependency
    graph.add_edge(
        ws_a,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("my-crate")
            .with_to_crate("platform-crate")
            .with_dependency_type(DependencyType::Normal)
            .with_target(Some("cfg(target_os = \"linux\")".to_string()))
            .build()
            .unwrap(),
    );

    let renderer = GraphRenderer::new(false, true);
    let mut output = Cursor::new(Vec::new());

    renderer.render_mermaid(&graph, &[], &mut output).unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Should include the crate-level detail
    assert!(result.contains("my-crate → platform-crate"));
}

#[test]
fn test_mermaid_node_shapes() -> miette::Result<()> {
    let mut graph = DiGraph::new();

    // Single crate workspace (should be stadium shape)
    let single_crate = graph.add_node(
        WorkspaceNode::builder()
            .with_name("single-crate-ws".to_string())
            .with_crates(vec!["single".to_string()])
            .build()
            .unwrap(),
    );

    // Large workspace (should be rectangle)
    let large_ws = graph.add_node(
        WorkspaceNode::builder()
            .with_name("large-workspace".to_string())
            .with_crates(vec![
                "crate1".to_string(),
                "crate2".to_string(),
                "crate3".to_string(),
                "crate4".to_string(),
                "crate5".to_string(),
                "crate6".to_string(),
            ])
            .build()
            .unwrap(),
    );

    // Medium workspace (should be rectangle)
    let medium_ws = graph.add_node(
        WorkspaceNode::builder()
            .with_name("medium-workspace".to_string())
            .with_crates(vec![
                "crate1".to_string(),
                "crate2".to_string(),
                "crate3".to_string(),
            ])
            .build()
            .unwrap(),
    );

    // Add some dependencies to create cycles
    graph.add_edge(
        single_crate,
        large_ws,
        DependencyEdge::builder()
            .with_from_crate("single")
            .with_to_crate("crate1")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        large_ws,
        medium_ws,
        DependencyEdge::builder()
            .with_from_crate("crate1")
            .with_to_crate("crate2")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        medium_ws,
        single_crate,
        DependencyEdge::builder()
            .with_from_crate("crate2")
            .with_to_crate("single")
            .with_dependency_type(DependencyType::Normal)
            .build()
            .unwrap(),
    );

    // Create a cycle for testing double circle shape
    let cycles = vec![
        WorkspaceCycle::builder()
            .add_edge()
            .from_workspace("single-crate-ws")
            .to_workspace("large-workspace")
            .from_crate("single")
            .to_crate("crate1")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("large-workspace")
            .to_workspace("medium-workspace")
            .from_crate("crate1")
            .to_crate("crate2")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("medium-workspace")
            .to_workspace("single-crate-ws")
            .from_crate("crate2")
            .to_crate("single")
            .dependency_type("Normal")
            .build()?,
    ];

    let renderer = GraphRenderer::new(true, false);
    let mut output = Cursor::new(Vec::new());

    renderer
        .render_mermaid(&graph, &cycles, &mut output)
        .unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();
    println!("Mermaid output for node shapes test:\n{result}");

    // Check node shapes based on the actual logic:
    // - Double circle: in cycle AND NOT (>5 crates)
    // - Stadium shape: single crate AND NOT in cycle
    // - Rectangle: default or >5 crates

    // Single crate in cycle should use double circle
    assert!(result.contains("single_crate_ws((\"single-crate-ws\"))"));

    // Large workspace (>5 crates) in cycle should use rectangle
    assert!(result.contains("large_workspace[\"large-workspace\"]"));

    // Medium workspace (3 crates) in cycle should use double circle
    assert!(result.contains("medium_workspace((\"medium-workspace\"))"));

    Ok(())
}

#[test]
fn test_mermaid_cycle_severity() -> miette::Result<()> {
    let mut graph = DiGraph::new();

    // Create a simple 2-workspace cycle (Low severity)
    let ws_a = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-a".to_string())
            .with_crates(vec!["crate-a".to_string()])
            .build()
            .unwrap(),
    );

    let ws_b = graph.add_node(
        WorkspaceNode::builder()
            .with_name("workspace-b".to_string())
            .with_crates(vec!["crate-b".to_string()])
            .build()
            .unwrap(),
    );

    // Add dev dependencies only (should be low severity)
    graph.add_edge(
        ws_a,
        ws_b,
        DependencyEdge::builder()
            .with_from_crate("crate-a")
            .with_to_crate("crate-b")
            .with_dependency_type(DependencyType::Dev)
            .build()
            .unwrap(),
    );

    graph.add_edge(
        ws_b,
        ws_a,
        DependencyEdge::builder()
            .with_from_crate("crate-b")
            .with_to_crate("crate-a")
            .with_dependency_type(DependencyType::Build)
            .build()
            .unwrap(),
    );

    let cycles = vec![
        WorkspaceCycle::builder()
            .add_edge()
            .from_workspace("workspace-a")
            .to_workspace("workspace-b")
            .from_crate("crate-a")
            .to_crate("crate-b")
            .dependency_type("Dev")
            .add_edge()?
            .from_workspace("workspace-b")
            .to_workspace("workspace-a")
            .from_crate("crate-b")
            .to_crate("crate-a")
            .dependency_type("Build")
            .build()?,
    ];

    let renderer = GraphRenderer::new(true, false);
    let mut output = Cursor::new(Vec::new());

    renderer
        .render_mermaid(&graph, &cycles, &mut output)
        .unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // Check that cycle severity is shown
    assert!(result.contains("subgraph CycleSeverity[\"Cycle Severity\"]"));

    // Low severity cycle should have single warning icon
    assert!(result.contains("⚠️ Cycle 1: 2 workspaces"));

    // Should show the workspace path
    assert!(result.contains("workspace-a → workspace-b"));

    Ok(())
}

#[test]
fn test_mermaid_high_severity_cycle() -> miette::Result<()> {
    let mut graph = DiGraph::new();

    // Create a 5-workspace cycle with normal dependencies (High severity)
    let ws1 = graph.add_node(
        WorkspaceNode::builder()
            .with_name("ws1".to_string())
            .with_crates(vec!["c1".to_string()])
            .build()
            .unwrap(),
    );
    let ws2 = graph.add_node(
        WorkspaceNode::builder()
            .with_name("ws2".to_string())
            .with_crates(vec!["c2".to_string()])
            .build()
            .unwrap(),
    );
    let ws3 = graph.add_node(
        WorkspaceNode::builder()
            .with_name("ws3".to_string())
            .with_crates(vec!["c3".to_string()])
            .build()
            .unwrap(),
    );
    let ws4 = graph.add_node(
        WorkspaceNode::builder()
            .with_name("ws4".to_string())
            .with_crates(vec!["c4".to_string()])
            .build()
            .unwrap(),
    );
    let ws5 = graph.add_node(
        WorkspaceNode::builder()
            .with_name("ws5".to_string())
            .with_crates(vec!["c5".to_string()])
            .build()
            .unwrap(),
    );

    // Create cycle edges
    let edges = vec![(ws1, ws2), (ws2, ws3), (ws3, ws4), (ws4, ws5), (ws5, ws1)];

    for (from, to) in edges {
        graph.add_edge(
            from,
            to,
            DependencyEdge::builder()
                .with_from_crate(&format!("c{}", from.index() + 1))
                .with_to_crate(&format!("c{}", to.index() + 1))
                .with_dependency_type(DependencyType::Normal)
                .build()
                .unwrap(),
        );
    }

    let cycles = vec![
        WorkspaceCycle::builder()
            .add_edge()
            .from_workspace("ws1")
            .to_workspace("ws2")
            .from_crate("c1")
            .to_crate("c2")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("ws2")
            .to_workspace("ws3")
            .from_crate("c2")
            .to_crate("c3")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("ws3")
            .to_workspace("ws4")
            .from_crate("c3")
            .to_crate("c4")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("ws4")
            .to_workspace("ws5")
            .from_crate("c4")
            .to_crate("c5")
            .dependency_type("Normal")
            .add_edge()?
            .from_workspace("ws5")
            .to_workspace("ws1")
            .from_crate("c5")
            .to_crate("c1")
            .dependency_type("Normal")
            .build()?,
    ];

    let renderer = GraphRenderer::new(true, false);
    let mut output = Cursor::new(Vec::new());

    renderer
        .render_mermaid(&graph, &cycles, &mut output)
        .unwrap();

    let result = String::from_utf8(output.into_inner()).unwrap();

    // High severity cycle should have three alert icons
    assert!(result.contains("🚨🚨🚨 Cycle 1: 5 workspaces"));

    Ok(())
}
