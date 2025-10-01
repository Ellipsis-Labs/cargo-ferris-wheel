# Repository Guidelines

## Project Structure & Module Organization

The CLI entrypoint lives in `src/main.rs` with argument parsing in `src/cli.rs`. Command orchestration sits under `src/commands/`, while domain logic is split across `src/executors/`, `src/analyzer/`, `src/detector/`, and `src/graph/`. Shared helpers are in `src/common.rs` and `src/utils/`. Reporting formats (human, JSON, JUnit, GitHub) live in `src/reports/`. Integration tests sit in `tests/`; repo-level config such as `rustfmt.toml`, `deny.toml`, and `lefthook.yml` is kept at the root.

## Build, Test, and Development Commands

Use `cargo build` for debug builds and `cargo build --release` when profiling. Run `cargo check --all-targets` plus `cargo clippy --all-targets -- -D warnings` before review. Format with `cargo fmt`. Execute `cargo nextest run --profile ci --cargo-profile ci` for the canonical test suite; keep `cargo test` for narrow repros. Guard dependencies with `cargo deny check`. For manual verification, `cargo run -- inspect` scans for cycles and `cargo run -- spectacle` previews graph output.

## Coding Style & Naming Conventions

Follow Rust 2021 defaults enforced by `rustfmt` (2-space indentation). Modules and files use snake_case; types and traits use PascalCase; public constants belong in `src/constants.rs`. Avoid `unwrap`; return typed errors with `thiserror` and surface them through `miette::Result`. Document subtle behaviors sparingly with short comments.

## Testing Guidelines

Place unit tests beside implementations via `mod tests`. Integration coverage lives in `tests/`, named `<feature>_test.rs`. Use `assert_cmd`, `predicates`, and `tempfile` for CLI scenarios, and match on concrete error variants. Every feature should pass `cargo nextest run --profile ci --cargo-profile ci`; refresh any fixtures or snapshots you modify.

## Commit & Pull Request Guidelines

Adopt Conventional Commit prefixes seen in history (e.g., `feat:`, `fix:`, `chore(deps):`). Keep subject lines under 72 characters. Each PR should link issues when available, list manual verification steps, and attach screenshots or CLI output for visualization changes. Confirm `cargo check`, `cargo nextest`, `cargo fmt`, and `cargo clippy` pass locally before requesting review, and call out intentional skips.

## Agent-Specific Tips

Coordinate with maintainers before altering `flake.nix` or `lefthook.yml`. When adding reports or executors, define toggles in `src/config/` and document them in `README.md`. Regenerate diagrams with `cargo run -- spectacle` and commit changed artifacts.
