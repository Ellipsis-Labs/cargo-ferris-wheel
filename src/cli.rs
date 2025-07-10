use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::common::{CommonArgs, CycleDisplayArgs, FormatArgs};

#[derive(Parser)]
#[command(
    bin_name = "cargo",
    subcommand_required = true,
    subcommand_precedence_over_arg = true,
    version
)]
pub struct CargoArgs {
    #[command(subcommand)]
    pub command: CargoCommand,
}

#[derive(Subcommand)]
pub enum CargoCommand {
    #[command(name = "ferris-wheel")]
    FerrisWheel(Cli),
}

#[derive(Parser)]
#[command(
    name = "ferris-wheel",
    about = "🎡 Detect workspace dependency cycles in Rust monorepos",
    long_about = "cargo-ferris-wheel analyzes your Rust workspace structure to find circular \
                  dependencies between workspaces. It includes all dependency types by default \
                  and provides multiple visualization options.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Inspect the carnival rides for dangerous cycles
    ///
    /// Analyzes your workspace dependency graph to find circular dependencies
    /// between workspaces. Circular dependencies prevent proper build ordering
    /// and can cause issues with tools like hakari. This command helps you
    /// identify and fix these cycles before they cause problems.
    #[command(
        long_about = "Analyze workspace dependencies to detect circular dependency chains. This \
                      command scans all Cargo.toml files in your workspace, builds a dependency \
                      graph, and uses Tarjan's algorithm to find strongly connected components \
                      (cycles). By default, it checks for cycles between workspaces, but can also \
                      check for cycles within individual workspaces using --intra-workspace."
    )]
    Inspect {
        #[command(flatten)]
        common: CommonArgs,

        #[command(flatten)]
        format: FormatArgs,

        #[command(flatten)]
        cycle_display: CycleDisplayArgs,

        /// Exit with error code if cycles found
        #[arg(long, env = "CARGO_FERRIS_WHEEL_ERROR_ON_CYCLES")]
        error_on_cycles: bool,

        /// Check for cycles within workspaces (intra-workspace) instead of
        /// between workspaces
        #[arg(long, env = "CARGO_FERRIS_WHEEL_INTRA_WORKSPACE")]
        intra_workspace: bool,
    },

    /// Create a spectacular visualization of your dependency carnival
    ///
    /// Generates visual representations of your workspace dependency graph
    /// in multiple formats. Perfect for documentation, debugging complex
    /// dependency relationships, or understanding your monorepo structure.
    #[command(
        long_about = "Generate visual dependency graphs in various formats including ASCII art, \
                      Mermaid diagrams, Graphviz DOT files, and D2 diagrams. The generated graphs \
                      show workspace relationships, highlight circular dependencies, and can \
                      include crate-level details. Use this to visualize and understand complex \
                      dependency structures in your monorepo."
    )]
    Spectacle {
        #[command(flatten)]
        common: CommonArgs,

        /// Graph format
        #[arg(
            short,
            long,
            value_enum,
            default_value = "ascii",
            env = "CARGO_FERRIS_WHEEL_GRAPH_FORMAT"
        )]
        format: GraphFormat,

        /// Output file (stdout if not specified)
        #[arg(short, long, env = "CARGO_FERRIS_WHEEL_OUTPUT")]
        output: Option<PathBuf>,

        /// Highlight cycles in the graph
        #[arg(
            long,
            default_value = "true",
            env = "CARGO_FERRIS_WHEEL_HIGHLIGHT_CYCLES"
        )]
        highlight_cycles: bool,

        /// Include crate-level details
        #[arg(long, env = "CARGO_FERRIS_WHEEL_SHOW_CRATES")]
        show_crates: bool,
    },

    /// Put a spotlight on cycles involving a specific crate
    ///
    /// Focuses the cycle detection on a specific crate, showing only the
    /// circular dependencies that involve that crate. Useful for debugging
    /// why a particular crate is part of a dependency cycle.
    #[command(
        long_about = "Analyze circular dependencies involving a specific crate. This command \
                      filters the cycle detection results to show only cycles that include the \
                      specified crate, making it easier to understand and fix issues with a \
                      particular component. Works for both inter-workspace and intra-workspace \
                      cycle detection."
    )]
    Spotlight {
        /// Name of the crate to analyze
        #[arg(value_name = "CRATE_NAME", env = "CARGO_FERRIS_WHEEL_CRATE_NAME")]
        crate_name: String,

        #[command(flatten)]
        common: CommonArgs,

        #[command(flatten)]
        format: FormatArgs,

        #[command(flatten)]
        cycle_display: CycleDisplayArgs,

        /// Check for cycles within workspaces (intra-workspace) instead of
        /// between workspaces
        #[arg(long, env = "CARGO_FERRIS_WHEEL_INTRA_WORKSPACE")]
        intra_workspace: bool,
    },

    /// See the full lineup of workspace dependencies
    ///
    /// Shows the dependency relationships between workspaces in your monorepo.
    /// Can display dependencies, reverse dependencies (dependents), and
    /// transitive dependencies to help you understand your project structure.
    #[command(
        long_about = "Display workspace dependency relationships in your monorepo. Shows which \
                      workspaces depend on others, and with --reverse, which workspaces are \
                      depended upon. The --transitive flag includes indirect dependencies. This \
                      is particularly useful for understanding the impact of changes and planning \
                      refactoring efforts."
    )]
    Lineup {
        /// Specific workspace to analyze (shows all workspaces if not
        /// specified)
        #[arg(
            long,
            value_name = "WORKSPACE_NAME",
            env = "CARGO_FERRIS_WHEEL_WORKSPACE"
        )]
        workspace: Option<String>,

        /// Show reverse dependencies (what depends on the specified workspace)
        #[arg(long, env = "CARGO_FERRIS_WHEEL_REVERSE")]
        reverse: bool,

        /// Include transitive dependencies (dependencies of dependencies)
        #[arg(long, env = "CARGO_FERRIS_WHEEL_TRANSITIVE")]
        transitive: bool,

        #[command(flatten)]
        common: CommonArgs,

        #[command(flatten)]
        format: FormatArgs,
    },

    /// Discover the ripple effects from changed files
    ///
    /// Analyzes which workspaces and crates are affected by changes to specific
    /// files. Essential for CI/CD pipelines to determine what needs to be
    /// rebuilt or retested based on file changes.
    #[command(
        long_about = "Determine which workspaces and crates are affected by file changes. This \
                      command maps changed files to their containing crates, then traces through \
                      the dependency graph to find all affected components. Perfect for \
                      optimizing CI pipelines by only building and testing what actually changed. \
                      Supports JSON output for easy integration."
    )]
    Ripples {
        /// List of changed files
        #[arg(
            required = true,
            value_name = "FILES",
            help = "Files that have changed",
            env = "CARGO_FERRIS_WHEEL_FILES"
        )]
        files: Vec<String>,

        /// Include crate-level information in output
        #[arg(long)]
        show_crates: bool,

        /// Include only directly affected crates (no reverse dependencies)
        #[arg(long, env = "CARGO_FERRIS_WHEEL_DIRECT_ONLY")]
        direct_only: bool,

        /// Exclude dev-dependencies from analysis
        #[arg(long, env = "CARGO_FERRIS_WHEEL_EXCLUDE_DEV")]
        exclude_dev: bool,

        /// Exclude build-dependencies from analysis
        #[arg(long, env = "CARGO_FERRIS_WHEEL_EXCLUDE_BUILD")]
        exclude_build: bool,

        /// Exclude target-specific dependencies
        #[arg(long, env = "CARGO_FERRIS_WHEEL_EXCLUDE_TARGET")]
        exclude_target: bool,

        #[command(flatten)]
        format: FormatArgs,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Junit,
    #[value(name = "github")]
    GitHub,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum GraphFormat {
    Ascii,
    Mermaid,
    Dot,
    D2,
}
