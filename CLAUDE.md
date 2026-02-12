# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

outlier is a Rust CLI tool and HTTP API server for calculating percentiles from numerical datasets. It supports JSON and CSV input formats.

## Common Commands

```bash
# Build (with server feature)
cargo build --features server

# Build release
cargo build --release --features server

# Run tests
cargo test

# Run a specific test
cargo test test_calculate_percentile_95th

# Run CLI
cargo run -- -v 1,2,3,4,5 -p 95

# Run API server (port 3000 by default)
cargo run --features server -- --serve

# Run API server on custom port
cargo run --features server -- --serve --port 8080

# Install locally
cargo install --features server --path .

# Volume testing
cargo run --example volume_test
cargo run --example volume_test -- --with-api  # requires server running
```

## Architecture

### Module Structure

- **src/lib.rs** - Core library with `calculate_percentile()` function and file parsing utilities. Exports public types (`CalculateRequest`, `CalculateResponse`, `ErrorResponse`) used by both CLI and server.
- **src/main.rs** - CLI entrypoint using clap. Handles argument parsing and delegates to either CLI mode or server mode.
- **src/server.rs** - Axum-based HTTP API (behind `server` feature flag). Provides `/calculate`, `/calculate/file`, and `/health` endpoints with OpenAPI/Swagger docs at `/docs`.
- **src/telemetry.rs** - OpenTelemetry integration for Honeycomb tracing. Configurable via `HONEYCOMB_API_KEY` and `OTEL_SERVICE_NAME` env vars.

### Feature Flags

- `default` - CLI only (no server dependencies)
- `server` - Enables HTTP API server with axum, utoipa (OpenAPI), and Swagger UI

### Key Dependencies

- **clap** - CLI argument parsing with derive macros
- **axum** - HTTP server framework (optional, server feature)
- **utoipa** - OpenAPI spec generation (optional, server feature)
- **tracing/opentelemetry** - Distributed tracing to Honeycomb

## Releasing

Uses cargo-release for versioning:
```bash
cargo release patch --execute  # 0.2.6 → 0.2.7
cargo release minor --execute  # 0.2.6 → 0.3.0
```

## Development Workflow

Main branch is protected. All changes must follow this workflow:

1. **Create a GitHub issue** with context for the work being done
2. **Create a feature branch** from main (e.g., `fix/help-output`, `feat/new-endpoint`)
3. **Push to the feature branch** and open a PR referencing the issue
4. **CI must pass** before merging — required checks: Test, Clippy, Security Audit, Format
5. **Merge the PR** into main only after CI passes

Never push directly to main. Never force push to main.

### Branch Naming

Use the format `<linear-id>/<brief-description>` (e.g., `bea-35/dockerfile-container-publishing`). The Linear issue ID prefix enables automatic linking.

### PR Conventions

The PR body must reference the Linear issue ID (e.g., `Closes BEA-35`). This auto-closes the corresponding Linear issue when the PR is merged.

## Git Commit Guidelines

- Do NOT add "Co-Authored-By" lines to commit messages

## Personality

Act as a professional Rust developer. Be direct, precise, and focus on idiomatic Rust patterns and best practices.
