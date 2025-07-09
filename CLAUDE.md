# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

cargo-ferris-wheel is a Rust CLI tool for detecting and visualizing circular dependencies in Rust monorepos. It uses carnival-themed command names (inspect, spectacle, ripples, lineup, spotlight) to make dependency analysis more engaging.

## Common Development Commands

### Building and Testing

```bash
# Build
cargo build                    # Debug build
cargo build --release         # Release build

# Test
cargo test                    # Run all tests
cargo nextest run --profile ci --cargo-profile ci  # CI testing

# Code Quality
cargo check --all-targets     # Type checking
cargo clippy --all-targets -- -D warnings  # Linting
cargo fmt                     # Format code
cargo deny check              # License compliance
cargo audit                   # Security audit
```

### Running the Tool

```bash
cargo run -- inspect          # Check for circular dependencies
cargo run -- spectacle        # Generate dependency graph visualization
cargo run -- ripples --files src/lib.rs  # Find affected workspaces
cargo run -- lineup           # List all dependencies
cargo run -- spotlight <crate>  # Analyze specific crate
```

## Architecture

The codebase follows a layered architecture:

1. **CLI Layer** (`cli.rs`): Command-line parsing using clap
2. **Commands** (`commands/`): Command orchestration
3. **Executors** (`executors/`): Business logic for each command
4. **Core** (`analyzer/`, `detector/`, `graph/`):
   - Workspace analysis using cargo metadata
   - Cycle detection using Tarjan's algorithm
   - Graph construction with petgraph
5. **Reports** (`reports/`): Output formatting (human, json, junit, github)

Key patterns:

- Parallel processing with Rayon for performance
- Comprehensive error handling with miette
- Strong typing with custom error types
- Multiple output formats for CI integration

## Key Implementation Details

- **Dependency Classification**: The analyzer distinguishes between direct, dev, and build dependencies
- **Cycle Detection**: Uses Tarjan's strongly connected components algorithm for efficiency
- **Graph Visualization**: Supports DOT, Mermaid, and ASCII output formats
- **Progress Indication**: Uses indicatif for user feedback during analysis
- **File Change Analysis**: Can determine affected workspaces from file changes

## Testing Approach

Tests are located alongside implementation files and in `tests/`. Run individual tests with:

```bash
cargo test test_name
cargo test --test integration_test_name
```

The project uses assert_cmd and predicates for CLI testing, with tempfile for test isolation.

## Development Best Practices

- Always use `cargo nextest run` instead of `cargo test` for running tests

## Error Handling Guidelines

cargo-ferris-wheel uses a carefully chosen combination of error handling crates that balance developer ergonomics with exceptional user experience:

- [`thiserror`](https://crates.io/crates/thiserror): For defining strongly-typed, zero-cost error enums with automatic trait implementations
- [`miette`](https://crates.io/crates/miette): For rich, compiler-quality error diagnostics in user-facing applications
- [`anyhow`](https://crates.io/crates/anyhow): Legacy - being phased out in favor of miette

**The recommendation is to use `thiserror` + `miette` for all new code.**

### Bottom Line Up Front

- **Libraries**: Define concrete error types with `thiserror`, return `Result<T, YourError>`
- **Applications**: Use `miette::Result<T>` at the top level for beautiful error reporting
- **Never** use `unwrap()` - use `expect()` only when certain a condition is infallible
- **Always** add context when propagating errors with `.wrap_err()`
- **Leverage** miette's diagnostic features for user-facing errors

### Error Type Definitions

Use `thiserror` to define error enums with zero boilerplate:

```rust
use thiserror::Error;
use miette::{Diagnostic, SourceSpan};

#[derive(Error, Debug, Diagnostic)]
pub enum AnalyzerError {
    #[error("Circular dependency detected")]
    #[diagnostic(
        code(ferris_wheel::analyzer::circular_dependency),
        help("Remove one of the dependencies in the cycle to break it")
    )]
    CircularDependency {
        cycle: Vec<String>,
        #[source]
        source: detector::CycleError,
    },

    #[error("Invalid workspace structure")]
    #[diagnostic(code(ferris_wheel::analyzer::invalid_workspace))]
    InvalidWorkspace {
        #[source_code]
        manifest: String,
        #[label("error occurs here")]
        span: SourceSpan,
    },
}
```

### Error Handling Patterns

#### Library Functions

```rust
// ✅ Good: Concrete error type with context
pub fn analyze_workspace(path: &Path) -> Result<Analysis, AnalyzerError> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(path)
        .exec()
        .map_err(|e| AnalyzerError::MetadataFailed {
            path: path.to_owned(),
            source: e,
        })?;

    // ... analysis logic
}
```

#### Application Entry Points

```rust
use miette::{Result, IntoDiagnostic, WrapErr};

fn main() -> Result<()> {
    miette::set_panic_hook();

    let args = Args::parse()
        .into_diagnostic()
        .wrap_err("Failed to parse command line arguments")?;

    match args.command {
        Command::Inspect => inspect_command()
            .wrap_err("Inspection failed")?,
        // ... other commands
    }

    Ok(())
}
```

#### Adding Context Through the Stack

```rust
fn process_dependencies(workspace: &Workspace) -> miette::Result<DependencyGraph> {
    let packages = analyze_packages(&workspace.packages)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to analyze {} packages", workspace.packages.len()))?;

    let graph = build_dependency_graph(packages)
        .map_err(|e| GraphError::BuildFailed {
            workspace_name: workspace.name.clone(),
            source: Box::new(e),
        })
        .into_diagnostic()?;

    Ok(graph)
}
```

### Testing Error Handling

Never convert errors to strings for testing! Always match on the actual error type:

```rust
#[test]
fn test_circular_dependency_detection() {
    let result = analyze_workspace("test-fixtures/circular");

    // ✅ GOOD: Use matches! macro
    assert!(matches!(
        result,
        Err(AnalyzerError::CircularDependency { cycle, .. }) if cycle.len() > 2
    ));

    // ❌ BAD: Don't convert to string!
    // assert!(result.unwrap_err().to_string().contains("circular"));
}
```

### Common Pitfalls to Avoid

1. **Over-wrapping Errors**: Only add meaningful context

```rust
// ❌ DON'T: Add redundant context
let content = fs::read_to_string(path)
    .into_diagnostic()
    .wrap_err("Failed to read file")  // Already in IO error
    .wrap_err("File operation failed")?;  // Too generic

// ✅ DO: Add meaningful context only
let content = fs::read_to_string(path)
    .into_diagnostic()
    .wrap_err_with(|| format!("Failed to load Cargo.toml from '{}'", path.display()))?;
```

2. **Using unwrap() in Production**: Always handle errors properly

```rust
// ❌ DON'T: Use unwrap
let metadata = get_metadata().unwrap();

// ✅ DO: Handle errors
let metadata = get_metadata()
    .wrap_err("Failed to load workspace metadata")?;
```

3. **Losing Error Information**: Preserve error structure

```rust
// ❌ DON'T: Convert to strings
fn process() -> Result<(), String> {
    operation().map_err(|e| e.to_string())?;
}

// ✅ DO: Preserve structure
fn process() -> miette::Result<()> {
    operation()
        .into_diagnostic()
        .wrap_err("Operation failed")?;
}
```
