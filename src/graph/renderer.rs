use std::collections::{BTreeMap, HashMap};
use std::io::Write;

use miette::Result;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::detector::WorkspaceCycle;
use crate::error::FerrisWheelError;
use crate::graph::{DependencyEdge, DependencyType, WorkspaceNode};

// Blue-Orange Accessible Palette - Soothing colors with excellent contrast
mod colors {
    pub const NORMAL_NODE_FILL: &str = "#E3F2FD"; // Light blue
    pub const NORMAL_NODE_STROKE: &str = "#1976D2"; // Medium blue
    pub const CYCLE_NODE_FILL: &str = "#FFF3E0"; // Light orange
    pub const CYCLE_NODE_STROKE: &str = "#F57C00"; // Vibrant orange
    pub const NORMAL_EDGE: &str = "#64B5F6"; // Soft blue
    pub const DEV_EDGE: &str = "#90A4AE"; // Blue-grey
    pub const BUILD_EDGE: &str = "#81C784"; // Soft green
    pub const CYCLE_EDGE: &str = "#FF6500"; // Deep orange
    pub const LEGEND_BG: &str = "#FAFAFA"; // Off-white background
}

// Helper macro for write operations that converts IO errors
macro_rules! writeln_out {
    ($dst:expr) => {
        writeln!($dst).map_err(FerrisWheelError::from)
    };
    ($dst:expr, $($arg:tt)*) => {
        writeln!($dst, $($arg)*).map_err(FerrisWheelError::from)
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CycleSeverity {
    Low,    // 2 workspaces, mostly dev/build deps
    Medium, // 3-4 workspaces or mix of dependency types
    High,   // 5+ workspaces or mostly normal deps
}

pub struct GraphRenderer {
    highlight_cycles: bool,
    show_crates: bool,
}

impl GraphRenderer {
    pub fn new(highlight_cycles: bool, show_crates: bool) -> Self {
        Self {
            highlight_cycles,
            show_crates,
        }
    }

    pub fn render_ascii(
        &self,
        graph: &DiGraph<WorkspaceNode, DependencyEdge>,
        cycles: &[WorkspaceCycle],
        output: &mut dyn Write,
    ) -> Result<()> {
        if graph.node_count() == 0 {
            writeln_out!(output, "No workspaces found to visualize")?;
            return Ok(());
        }

        writeln_out!(output, "\n📊 Workspace Dependency Graph\n")?;

        // Build sets of workspace names involved in cycles for easy lookup
        let cycles_ws_names: Vec<Vec<String>> = cycles
            .iter()
            .map(|cycle| cycle.workspace_names().to_vec())
            .collect();

        // Sort nodes by name for consistent output
        let mut nodes: Vec<NodeIndex> = graph.node_indices().collect();
        nodes.sort_by_key(|&idx| graph[idx].name());

        for node_idx in nodes {
            let node = &graph[node_idx];
            let ws_name = node.name();

            // Check if this workspace is involved in any cycle
            let in_cycle = cycles_ws_names
                .iter()
                .any(|cycle| cycle.iter().any(|c| c == ws_name));

            // Print workspace header with cycle indicator
            if in_cycle && self.highlight_cycles {
                writeln_out!(output, "┌─────────────────────────────────────┐")?;
                writeln_out!(output, "│ {} ⚠️  IN CYCLE", ws_name)?;
                writeln_out!(output, "└─────────────────────────────────────┘")?;
            } else {
                writeln_out!(output, "{}", ws_name)?;
            }

            // Show crates in this workspace if requested
            if self.show_crates && !node.crates().is_empty() {
                writeln_out!(output, "  📦 Crates: {}", node.crates().join(", "))?;
            }

            // Aggregate edges by target and dependency type
            type EdgeKey = (NodeIndex, DependencyType);
            let mut edge_groups: HashMap<EdgeKey, Vec<&DependencyEdge>> = HashMap::new();

            for edge in graph.edges(node_idx) {
                let edge_data = edge.weight();
                let key = (edge.target(), edge_data.dependency_type().clone());
                edge_groups.entry(key).or_default().push(edge_data);
            }

            if edge_groups.is_empty() {
                writeln_out!(output, "  └── (no cross-workspace dependencies)")?;
            } else {
                // Sort groups by target workspace name and dependency type
                let mut groups: Vec<_> = edge_groups.into_iter().collect();
                groups.sort_by_key(|((target_idx, dep_type), _)| {
                    (graph[*target_idx].name(), dep_type.clone())
                });

                for (i, ((target_idx, dep_type), edges)) in groups.iter().enumerate() {
                    let target_node = &graph[*target_idx];
                    let is_last = i == groups.len() - 1;
                    let prefix = if is_last { "└──" } else { "├──" };

                    // Check if this edge is part of a cycle
                    let edge_in_cycle =
                        self.is_edge_in_cycle(ws_name, target_node.name(), &cycles_ws_names);

                    // Format the dependency line
                    let cycle_marker = if edge_in_cycle && self.highlight_cycles {
                        " ⚠️  [CYCLE]"
                    } else {
                        ""
                    };

                    let dep_type_str = match dep_type {
                        DependencyType::Normal => "normal",
                        DependencyType::Dev => "dev",
                        DependencyType::Build => "build",
                    };

                    let count_str = if edges.len() > 1 {
                        format!(" ({} {} deps)", edges.len(), dep_type_str)
                    } else {
                        format!(" ({dep_type_str})")
                    };

                    writeln_out!(
                        output,
                        "  {} → {}{}{}",
                        prefix,
                        target_node.name(),
                        count_str,
                        cycle_marker
                    )?;

                    // Show crate-level dependency details if requested
                    if self.show_crates {
                        let detail_prefix = if is_last { "      " } else { "  │   " };
                        for (j, edge) in edges.iter().enumerate() {
                            let is_last_detail = j == edges.len() - 1;
                            writeln_out!(
                                output,
                                "{}{}── {} → {} ({})",
                                detail_prefix,
                                if is_last_detail { "└" } else { "├" },
                                edge.from_crate(),
                                edge.to_crate(),
                                edge.target().unwrap_or("all targets")
                            )?;
                        }
                    }
                }
            }

            writeln_out!(output)?; // Empty line between workspaces
        }

        // Add legend if there are cycles
        if !cycles.is_empty() && self.highlight_cycles {
            writeln_out!(output, "⚠️  = Part of a dependency cycle")?;
        }

        Ok(())
    }

    pub fn render_mermaid(
        &self,
        graph: &DiGraph<WorkspaceNode, DependencyEdge>,
        cycles: &[WorkspaceCycle],
        output: &mut dyn Write,
    ) -> Result<()> {
        writeln_out!(output, "graph TD")?;

        // Build sets of workspace names involved in cycles
        let cycles_ws_names: Vec<Vec<String>> = cycles
            .iter()
            .map(|cycle| cycle.workspace_names().to_vec())
            .collect();

        // Group workspaces by prefix for subgraphs
        let groups = self.group_workspaces_by_prefix(graph);
        let mut ungrouped_nodes: Vec<NodeIndex> = graph.node_indices().collect();

        // Render subgraphs
        for (prefix, nodes) in groups.iter() {
            writeln_out!(output)?;
            writeln_out!(
                output,
                "    subgraph {}_group[\"{}\"*]",
                self.mermaid_id(prefix),
                prefix
            )?;

            for &node in nodes {
                let ws = &graph[node];
                let in_cycle = cycles_ws_names
                    .iter()
                    .any(|cycle| cycle.iter().any(|c| c == ws.name()));

                let node_id = self.mermaid_id(ws.name());
                let label = if self.show_crates {
                    format!("{}\\n{} crates", ws.name(), ws.crates().len())
                } else {
                    ws.name().to_string()
                };

                // Create tooltip text for click events
                let tooltip = format!(
                    "Workspace: {} - Crates: {} - Total: {}",
                    ws.name(),
                    if ws.crates().len() <= 3 {
                        ws.crates().join(", ")
                    } else {
                        format!(
                            "{}, ... ({} total)",
                            ws.crates()[..3].join(", "),
                            ws.crates().len()
                        )
                    },
                    ws.crates().len()
                );

                // Use different shapes based on workspace characteristics
                let node_shape = if ws.crates().len() > 5 {
                    format!("{node_id}[\"{label}\"]") // Rectangle for large workspaces (even in cycles)
                } else if in_cycle && self.highlight_cycles {
                    format!("{node_id}((\"{label}\"))") // Double circle for cycles
                } else if ws.crates().len() == 1 {
                    format!("{node_id}([\"{label}\"])") // Stadium shape for single-crate workspaces
                } else {
                    format!("{node_id}[\"{label}\"]") // Default rectangle
                };
                writeln_out!(output, "        {}", node_shape)?;
                writeln_out!(output, "        click {} \"{}\"", node_id, tooltip)?;

                if in_cycle && self.highlight_cycles {
                    writeln_out!(
                        output,
                        "        style {} fill:{},stroke:{},stroke-width:3px",
                        node_id,
                        colors::CYCLE_NODE_FILL,
                        colors::CYCLE_NODE_STROKE
                    )?;
                } else {
                    writeln_out!(
                        output,
                        "        style {} fill:{},stroke:{},stroke-width:2px",
                        node_id,
                        colors::NORMAL_NODE_FILL,
                        colors::NORMAL_NODE_STROKE
                    )?;
                }

                // Remove from ungrouped nodes
                ungrouped_nodes.retain(|&n| n != node);
            }

            writeln_out!(output, "    end")?;
        }

        // Render ungrouped nodes
        if !ungrouped_nodes.is_empty() {
            writeln_out!(output)?;
            for node in ungrouped_nodes {
                let ws = &graph[node];
                let in_cycle = cycles_ws_names
                    .iter()
                    .any(|cycle| cycle.iter().any(|c| c == ws.name()));

                let node_id = self.mermaid_id(ws.name());
                let label = if self.show_crates {
                    format!("{}\\n{} crates", ws.name(), ws.crates().len())
                } else {
                    ws.name().to_string()
                };

                // Create tooltip text for click events
                let tooltip = format!(
                    "Workspace: {} - Crates: {} - Total: {}",
                    ws.name(),
                    if ws.crates().len() <= 3 {
                        ws.crates().join(", ")
                    } else {
                        format!(
                            "{}, ... ({} total)",
                            ws.crates()[..3].join(", "),
                            ws.crates().len()
                        )
                    },
                    ws.crates().len()
                );

                // Use different shapes based on workspace characteristics
                let node_shape = if ws.crates().len() > 5 {
                    format!("    {node_id}[\"{label}\"]") // Rectangle for large workspaces (even in cycles)
                } else if in_cycle && self.highlight_cycles {
                    format!("    {node_id}((\"{label}\"))") // Double circle for cycles
                } else if ws.crates().len() == 1 {
                    format!("    {node_id}([\"{label}\"])") // Stadium shape for single-crate workspaces
                } else {
                    format!("    {node_id}[\"{label}\"]") // Default rectangle
                };
                writeln_out!(output, "{}", node_shape)?;
                writeln_out!(output, "    click {} \"{}\"", node_id, tooltip)?;

                if in_cycle && self.highlight_cycles {
                    writeln_out!(
                        output,
                        "    style {} fill:{},stroke:{},stroke-width:3px",
                        node_id,
                        colors::CYCLE_NODE_FILL,
                        colors::CYCLE_NODE_STROKE
                    )?;
                } else {
                    writeln_out!(
                        output,
                        "    style {} fill:{},stroke:{},stroke-width:2px",
                        node_id,
                        colors::NORMAL_NODE_FILL,
                        colors::NORMAL_NODE_STROKE
                    )?;
                }
            }
        }

        writeln_out!(output)?;

        // Aggregate edges by source, target, and dependency type
        type EdgeKey = (NodeIndex, NodeIndex, DependencyType);
        let mut edge_groups: HashMap<EdgeKey, Vec<&DependencyEdge>> = HashMap::new();

        for edge in graph.edge_indices() {
            let (source, target) = graph.edge_endpoints(edge).ok_or_else(|| {
                crate::error::FerrisWheelError::GraphError {
                    message: "Edge must have endpoints".to_string(),
                }
            })?;
            let edge_data = graph.edge_weight(edge).ok_or_else(|| {
                crate::error::FerrisWheelError::GraphError {
                    message: "Edge weight not found for existing edge".to_string(),
                }
            })?;
            let key = (source, target, edge_data.dependency_type().clone());
            edge_groups.entry(key).or_default().push(edge_data);
        }

        // Render aggregated edges
        for (link_style_index, ((source, target, dep_type), edges)) in
            edge_groups.into_iter().enumerate()
        {
            let source_ws = &graph[source];
            let target_ws = &graph[target];

            let edge_in_cycle =
                self.is_edge_in_cycle(source_ws.name(), target_ws.name(), &cycles_ws_names);

            let label = if self.show_crates {
                // Show all crate pairs when show_crates is true
                let pairs: Vec<String> = edges
                    .iter()
                    .map(|e| format!("{} → {}", e.from_crate(), e.to_crate()))
                    .collect();
                if pairs.len() > 1 {
                    let type_icon = match dep_type {
                        DependencyType::Normal => "📦",
                        DependencyType::Dev => "🔧",
                        DependencyType::Build => "🏗️",
                    };
                    format!(
                        "{} {} ({})",
                        type_icon,
                        pairs.len(),
                        format!("{dep_type:?}").to_lowercase()
                    )
                } else {
                    pairs[0].clone()
                }
            } else {
                // When not showing crates, use icons and cleaner labels
                let (icon, type_label) = match dep_type {
                    DependencyType::Normal => ("📦", "uses"),
                    DependencyType::Dev => ("🔧", "dev"),
                    DependencyType::Build => ("🏗️", "build"),
                };
                if edges.len() > 1 {
                    format!("{} {} {}", icon, edges.len(), type_label)
                } else {
                    format!("{icon} {type_label}")
                }
            };

            // Choose arrow type based on dependency type
            let arrow_type = match dep_type {
                DependencyType::Normal => "-->", // Solid arrow for normal deps
                DependencyType::Dev => "-.->",   // Dotted arrow for dev deps
                DependencyType::Build => "===>", // Thick arrow for build deps
            };

            if edge_in_cycle && self.highlight_cycles {
                writeln_out!(
                    output,
                    "    {} {}|{}| {}",
                    self.mermaid_id(source_ws.name()),
                    arrow_type,
                    label,
                    self.mermaid_id(target_ws.name())
                )?;
                writeln_out!(
                    output,
                    "    linkStyle {} stroke:{},stroke-width:3px",
                    link_style_index,
                    colors::CYCLE_EDGE
                )?;
            } else {
                writeln_out!(
                    output,
                    "    {} {}|{}| {}",
                    self.mermaid_id(source_ws.name()),
                    arrow_type,
                    label,
                    self.mermaid_id(target_ws.name())
                )?;
                // Color edges based on dependency type
                let edge_color = match dep_type {
                    DependencyType::Normal => colors::NORMAL_EDGE,
                    DependencyType::Dev => colors::DEV_EDGE,
                    DependencyType::Build => colors::BUILD_EDGE,
                };
                writeln_out!(
                    output,
                    "    linkStyle {} stroke:{},stroke-width:2px",
                    link_style_index,
                    edge_color
                )?;
            }
        }

        // Add legend
        if !cycles.is_empty() && self.highlight_cycles {
            writeln_out!(output)?;
            writeln_out!(output, "    subgraph Legend")?;
            writeln_out!(output, "        L1[Normal Workspace]")?;
            writeln_out!(output, "        L2[Workspace in Cycle]")?;
            writeln_out!(
                output,
                "        style L1 fill:{},stroke:{},stroke-width:2px",
                colors::NORMAL_NODE_FILL,
                colors::NORMAL_NODE_STROKE
            )?;
            writeln_out!(
                output,
                "        style L2 fill:{},stroke:{},stroke-width:3px",
                colors::CYCLE_NODE_FILL,
                colors::CYCLE_NODE_STROKE
            )?;
            writeln_out!(
                output,
                "        style Legend fill:{},stroke:#ddd,stroke-width:1px",
                colors::LEGEND_BG
            )?;
            writeln_out!(output, "    end")?;

            // Add cycle severity information
            writeln_out!(output)?;
            writeln_out!(output, "    subgraph CycleSeverity[\"Cycle Severity\"]")?;
            for (i, cycle) in cycles.iter().enumerate() {
                let severity = self.calculate_cycle_severity(cycle);
                let severity_icon = match severity {
                    CycleSeverity::Low => "⚠️",
                    CycleSeverity::Medium => "⚠️⚠️",
                    CycleSeverity::High => "🚨🚨🚨",
                };
                let workspace_list = cycle.workspace_names().join(" → ");
                writeln_out!(
                    output,
                    "        CS{}[\"{} Cycle {}: {} workspaces<br/>{}\"]",
                    i + 1,
                    severity_icon,
                    i + 1,
                    cycle.workspace_names().len(),
                    workspace_list
                )?;
            }
            writeln_out!(
                output,
                "        style CycleSeverity fill:{},stroke:#ddd,stroke-width:1px",
                colors::LEGEND_BG
            )?;
            writeln_out!(output, "    end")?;
        }

        Ok(())
    }

    pub fn render_dot(
        &self,
        graph: &DiGraph<WorkspaceNode, DependencyEdge>,
        cycles: &[WorkspaceCycle],
        output: &mut dyn Write,
    ) -> Result<()> {
        writeln_out!(output, "digraph workspace_dependencies {{")?;
        writeln_out!(output, "    rankdir=LR;")?;
        writeln_out!(output, "    node [shape=box, style=rounded];")?;
        writeln_out!(output)?;

        // Build sets of workspace names involved in cycles
        let cycles_ws_names: Vec<Vec<String>> = cycles
            .iter()
            .map(|cycle| cycle.workspace_names().to_vec())
            .collect();

        // Define nodes
        for node in graph.node_indices() {
            let ws = &graph[node];
            let in_cycle = cycles_ws_names
                .iter()
                .any(|cycle| cycle.iter().any(|c| c == ws.name()));

            let (fill_color, stroke_color) = if in_cycle && self.highlight_cycles {
                (colors::CYCLE_NODE_FILL, colors::CYCLE_NODE_STROKE)
            } else {
                (colors::NORMAL_NODE_FILL, colors::NORMAL_NODE_STROKE)
            };

            let label = if self.show_crates {
                format!("{}\\n{} crates", ws.name(), ws.crates().len())
            } else {
                ws.name().to_string()
            };

            writeln_out!(
                output,
                r#"    "{}" [label="{}", style=filled, fillcolor="{}", color="{}", penwidth=2];"#,
                ws.name(),
                label,
                fill_color,
                stroke_color
            )?;
        }

        writeln_out!(output)?;

        // Aggregate edges by source, target, and dependency type
        type EdgeKey = (NodeIndex, NodeIndex, DependencyType);
        let mut edge_groups: HashMap<EdgeKey, Vec<&DependencyEdge>> = HashMap::new();

        for edge in graph.edge_indices() {
            let (source, target) = graph.edge_endpoints(edge).ok_or_else(|| {
                crate::error::FerrisWheelError::GraphError {
                    message: "Edge must have endpoints".to_string(),
                }
            })?;
            let edge_data = graph.edge_weight(edge).ok_or_else(|| {
                crate::error::FerrisWheelError::GraphError {
                    message: "Edge weight not found for existing edge".to_string(),
                }
            })?;
            let key = (source, target, edge_data.dependency_type().clone());
            edge_groups.entry(key).or_default().push(edge_data);
        }

        // Render aggregated edges
        for ((source, target, dep_type), edges) in edge_groups {
            let source_ws = &graph[source];
            let target_ws = &graph[target];

            let edge_in_cycle =
                self.is_edge_in_cycle(source_ws.name(), target_ws.name(), &cycles_ws_names);

            let label = if self.show_crates {
                // Show all crate pairs when show_crates is true
                let pairs: Vec<String> = edges
                    .iter()
                    .map(|e| format!("{} → {}", e.from_crate(), e.to_crate()))
                    .collect();
                if pairs.len() > 1 {
                    format!("{:?} - {} deps", dep_type, pairs.len())
                } else {
                    pairs[0].clone()
                }
            } else {
                // When not showing crates, aggregate by type and count
                if edges.len() > 1 {
                    format!("{:?} - {} deps", dep_type, edges.len())
                } else {
                    format!("{dep_type:?}")
                }
            };

            if edge_in_cycle && self.highlight_cycles {
                writeln_out!(
                    output,
                    r#"    "{}" -> "{}" [label="{}", color="{}", penwidth=3];"#,
                    source_ws.name(),
                    target_ws.name(),
                    label,
                    colors::CYCLE_EDGE
                )?;
            } else {
                let edge_color = match dep_type {
                    DependencyType::Normal => colors::NORMAL_EDGE,
                    DependencyType::Dev => colors::DEV_EDGE,
                    DependencyType::Build => colors::BUILD_EDGE,
                };
                writeln_out!(
                    output,
                    r#"    "{}" -> "{}" [label="{}", color="{}", penwidth=2];"#,
                    source_ws.name(),
                    target_ws.name(),
                    label,
                    edge_color
                )?;
            }
        }

        writeln_out!(output, "}}")?;
        Ok(())
    }

    pub fn render_d2(
        &self,
        graph: &DiGraph<WorkspaceNode, DependencyEdge>,
        cycles: &[WorkspaceCycle],
        output: &mut dyn Write,
    ) -> Result<()> {
        writeln_out!(output, "# Workspace Dependency Graph\n")?;

        // Build sets of workspace names involved in cycles
        let cycles_ws_names: Vec<Vec<String>> = cycles
            .iter()
            .map(|cycle| cycle.workspace_names().to_vec())
            .collect();

        // Define nodes
        for node in graph.node_indices() {
            let ws = &graph[node];
            let in_cycle = cycles_ws_names
                .iter()
                .any(|cycle| cycle.iter().any(|c| c == ws.name()));

            let shape = if in_cycle && self.highlight_cycles {
                "hexagon"
            } else {
                "rectangle"
            };

            let label = if self.show_crates {
                format!("{}\\n{} crates", ws.name(), ws.crates().len())
            } else {
                ws.name().to_string()
            };

            writeln_out!(output, "{}: {} {{", self.d2_id(ws.name()), label)?;
            writeln_out!(output, "  shape: {}", shape)?;
            writeln_out!(
                output,
                "  style.fill: \"{}\"",
                if in_cycle && self.highlight_cycles {
                    colors::CYCLE_NODE_FILL
                } else {
                    colors::NORMAL_NODE_FILL
                }
            )?;
            writeln_out!(
                output,
                "  style.stroke: \"{}\"",
                if in_cycle && self.highlight_cycles {
                    colors::CYCLE_NODE_STROKE
                } else {
                    colors::NORMAL_NODE_STROKE
                }
            )?;
            writeln_out!(output, "}}")?;
            writeln_out!(output)?;
        }

        // Aggregate edges by source, target, and dependency type
        type EdgeKey = (NodeIndex, NodeIndex, DependencyType);
        let mut edge_groups: HashMap<EdgeKey, Vec<&DependencyEdge>> = HashMap::new();

        for edge in graph.edge_indices() {
            let (source, target) = graph.edge_endpoints(edge).ok_or_else(|| {
                crate::error::FerrisWheelError::GraphError {
                    message: "Edge must have endpoints".to_string(),
                }
            })?;
            let edge_data = graph.edge_weight(edge).ok_or_else(|| {
                crate::error::FerrisWheelError::GraphError {
                    message: "Edge weight not found for existing edge".to_string(),
                }
            })?;
            let key = (source, target, edge_data.dependency_type().clone());
            edge_groups.entry(key).or_default().push(edge_data);
        }

        // Render aggregated edges
        for ((source, target, dep_type), edges) in edge_groups {
            let source_ws = &graph[source];
            let target_ws = &graph[target];

            let edge_in_cycle =
                self.is_edge_in_cycle(source_ws.name(), target_ws.name(), &cycles_ws_names);

            let label = if self.show_crates {
                // Show all crate pairs when show_crates is true
                let pairs: Vec<String> = edges
                    .iter()
                    .map(|e| format!("{} → {}", e.from_crate(), e.to_crate()))
                    .collect();
                if pairs.len() > 1 {
                    format!("{:?} - {} deps", dep_type, pairs.len())
                } else {
                    pairs[0].clone()
                }
            } else {
                // When not showing crates, aggregate by type and count
                if edges.len() > 1 {
                    format!("{:?} - {} deps", dep_type, edges.len())
                } else {
                    format!("{dep_type:?}")
                }
            };

            writeln_out!(
                output,
                "{} -> {}: {} {{",
                self.d2_id(source_ws.name()),
                self.d2_id(target_ws.name()),
                label
            )?;

            if edge_in_cycle && self.highlight_cycles {
                writeln_out!(output, "  style.stroke: \"{}\"", colors::CYCLE_EDGE)?;
                writeln_out!(output, "  style.stroke-width: 3")?;
            } else {
                let edge_color = match dep_type {
                    DependencyType::Normal => colors::NORMAL_EDGE,
                    DependencyType::Dev => colors::DEV_EDGE,
                    DependencyType::Build => colors::BUILD_EDGE,
                };
                writeln_out!(output, "  style.stroke: \"{}\"", edge_color)?;
                writeln_out!(output, "  style.stroke-width: 2")?;
            }

            writeln_out!(output, "}}")?;
        }

        Ok(())
    }

    pub fn render_cycle_summary(
        &self,
        cycles: &[WorkspaceCycle],
        output: &mut dyn Write,
    ) -> Result<()> {
        writeln_out!(output, "\n🔄 Dependency Cycles Summary\n")?;

        if cycles.is_empty() {
            writeln_out!(output, "✅ No dependency cycles detected!")?;
            return Ok(());
        }

        for (i, cycle) in cycles.iter().enumerate() {
            let severity = self.calculate_cycle_severity(cycle);
            let severity_icon = match severity {
                CycleSeverity::Low => "⚠️",
                CycleSeverity::Medium => "⚠️",
                CycleSeverity::High => "🚨",
            };

            writeln_out!(
                output,
                "{} Cycle #{} (Severity: {:?})",
                severity_icon,
                i + 1,
                severity
            )?;
            writeln_out!(
                output,
                "  Workspaces: {}",
                cycle.workspace_names().join(" → ")
            )?;
            writeln_out!(output, "  Total edges in cycle: {}", cycle.edges().len())?;

            // Show dependency type breakdown
            let mut type_counts = std::collections::HashMap::new();
            for edge in cycle.edges() {
                *type_counts.entry(edge.dependency_type()).or_insert(0) += 1;
            }

            writeln_out!(output, "  Dependency types:")?;
            for (dep_type, count) in &type_counts {
                writeln_out!(output, "    - {}: {}", dep_type, count)?;
            }

            // Show edges by direction to understand the cycle better
            writeln_out!(output, "\n  📊 Edge breakdown by direction:")?;
            let mut directions: Vec<_> = cycle.edges_by_direction().keys().collect();
            directions.sort();

            for (from_ws, to_ws) in &directions {
                if let Some(edges) = cycle
                    .edges_by_direction()
                    .get(&(from_ws.to_string(), to_ws.to_string()))
                {
                    writeln_out!(output, "    {} → {}: {} edges", from_ws, to_ws, edges.len())?;
                }
            }

            // Suggest best edges to break
            writeln_out!(output, "\n  💡 Suggested break points:")?;
            let mut suggestions_found = false;

            // First, suggest dev/build dependencies as they're easier to break
            for (from_ws, to_ws) in &directions {
                if let Some(edges) = cycle
                    .edges_by_direction()
                    .get(&(from_ws.to_string(), to_ws.to_string()))
                {
                    let non_normal_edges: Vec<_> = edges
                        .iter()
                        .filter(|e| e.dependency_type() != "Normal")
                        .collect();

                    if !non_normal_edges.is_empty() {
                        suggestions_found = true;
                        writeln_out!(
                            output,
                            "     - {} → {} ({} dev/build dependencies)",
                            from_ws,
                            to_ws,
                            non_normal_edges.len()
                        )?;
                        if self.show_crates && non_normal_edges.len() <= 3 {
                            for edge in &non_normal_edges {
                                writeln_out!(
                                    output,
                                    "       • {} → {} ({})",
                                    edge.from_crate(),
                                    edge.to_crate(),
                                    edge.dependency_type()
                                )?;
                            }
                        }
                    }
                }
            }

            // If no dev/build dependencies, suggest the direction with fewer edges
            if !suggestions_found {
                let mut min_edges = usize::MAX;
                let mut best_direction = None;

                for (from_ws, to_ws) in &directions {
                    if let Some(edges) = cycle
                        .edges_by_direction()
                        .get(&(from_ws.to_string(), to_ws.to_string()))
                        && edges.len() < min_edges
                    {
                        min_edges = edges.len();
                        best_direction = Some((from_ws, to_ws));
                    }
                }

                if let Some((from_ws, to_ws)) = best_direction {
                    writeln_out!(
                        output,
                        "     - {} → {} ({} edges total)",
                        from_ws,
                        to_ws,
                        min_edges
                    )?;
                }
            }

            writeln_out!(output)?;
        }

        // Add general advice
        writeln_out!(output, "\n📝 General recommendations:")?;
        writeln_out!(
            output,
            "  • Focus on breaking dev/build dependencies first (easier to refactor)"
        )?;
        writeln_out!(
            output,
            "  • Consider extracting shared code into a separate workspace"
        )?;
        writeln_out!(
            output,
            "  • Break cycles at the point with the fewest dependencies"
        )?;

        Ok(())
    }

    fn is_edge_in_cycle(&self, from: &str, to: &str, cycles_ws_names: &[Vec<String>]) -> bool {
        // Check if both workspaces are in the same cycle
        // This will highlight ALL edges between workspaces that are part of a cycle
        cycles_ws_names
            .iter()
            .any(|cycle| cycle.contains(&from.to_string()) && cycle.contains(&to.to_string()))
    }

    fn mermaid_id(&self, name: &str) -> String {
        // Replace non-alphanumeric characters with underscores for valid Mermaid IDs
        name.chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect()
    }

    fn d2_id(&self, name: &str) -> String {
        // D2 supports more characters, but we'll quote if necessary
        if name.contains(' ') || name.contains('-') {
            format!("\"{name}\"")
        } else {
            name.to_string()
        }
    }

    // Group workspaces by common prefix (e.g., "atlas-" groups all atlas
    // workspaces)
    fn group_workspaces_by_prefix(
        &self,
        graph: &DiGraph<WorkspaceNode, DependencyEdge>,
    ) -> BTreeMap<String, Vec<NodeIndex>> {
        let mut groups: BTreeMap<String, Vec<NodeIndex>> = BTreeMap::new();

        for node in graph.node_indices() {
            let ws = &graph[node];
            // Extract prefix (everything before the first dash, or "other" if no dash)
            let prefix = if let Some(dash_pos) = ws.name().find('-') {
                ws.name()[..dash_pos].to_string()
            } else if ws.name().contains("workspace") {
                "workspace".to_string()
            } else {
                "other".to_string()
            };

            groups.entry(prefix).or_default().push(node);
        }

        // Only keep groups with more than one workspace
        groups.retain(|_, nodes| nodes.len() > 1);
        groups
    }

    fn calculate_cycle_severity(&self, cycle: &WorkspaceCycle) -> CycleSeverity {
        let workspace_count = cycle.workspace_names().len();
        let edges = cycle.edges();

        // Count dependency types
        let mut normal_deps = 0;
        let mut dev_deps = 0;
        let mut build_deps = 0;

        for edge in edges {
            match edge.dependency_type() {
                "Normal" => normal_deps += 1,
                "Dev" => dev_deps += 1,
                "Build" => build_deps += 1,
                _ => {}
            }
        }

        // Calculate severity based on workspace count and dependency types
        if workspace_count >= 5 || (normal_deps > dev_deps + build_deps) {
            CycleSeverity::High
        } else if workspace_count >= 3 || normal_deps > 0 {
            CycleSeverity::Medium
        } else {
            CycleSeverity::Low
        }
    }
}
