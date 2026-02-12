# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.3.5] - 2026-02-12

### Fixed
- Fixed help output not displaying when running `outlier` with no arguments. Replaced early-exit arg counting logic with a clean post-parse check.

## [0.3.4] - 2026-02-12

### Fixed
- Fixed server startup crash caused by duplicate global tracing subscriber initialization. Telemetry init is now only called in CLI mode.

## [0.3.3] - 2026-02-12

### Changed
- Updated OpenTelemetry stack from 0.27 to 0.31 and tonic from 0.12 to 0.14, with refactored telemetry shutdown using `OnceLock`.
- Updated axum to 0.8.8 and utoipa-swagger-ui to 9.0.2.
- Updated anyhow to 1.0.101, clap to 4.5.58, toml to 1.0, and reqwest to 0.13.

## [0.3.2] - 2026-02-12

### Added
- Test case to verify the `10_000_000` value input limit.

### Changed
- Refactored tests from an inline module in `src/lib.rs` to a separate `src/tests.rs` file for better code organization.

## [0.3.1] - 2026-01-16

### Added
- Additional example configuration files for different deployment scenarios:
  - `config.production.toml`: Production setup with JSON logging to file, localhost binding, port 8080
  - `config.development.toml`: Development setup with debug logging and pretty format, port 3000
  - `config.minimal.toml`: Minimal configuration demonstrating defaults override

## [0.3.0] - 2026-01-16

### Added
- **TOML Configuration File Support**: Server mode now accepts configuration files
  - `--config` / `-c` CLI flag for specifying config file path
  - `CONFIG_FILE` environment variable support
  - Configurable logging: level (trace/debug/info/warn/error), output (stdout/stderr/file), format (compact/pretty/json)
  - Configurable server: port and bind IP address
  - CLI flags override config file settings
  - Example configuration file (`config.example.toml`)
  - Falls back to sensible defaults if no config file provided

## [0.2.6] - 2026-01-16

### Added
- SECURITY.md with GitHub private vulnerability reporting for security disclosures

## [0.2.5] - 2026-01-16

### Fixed
- Fixed cargo-release tag naming to use `v0.2.x` instead of `vv0.2.x`
- Fixed duplicate changelog entries by letting cargo-release manage version headers

## [0.2.4] - 2026-01-16

### Changed
- Skip telemetry initialization for `--version`/`-V` and `--help`/`-h` flags for faster startup
- Display help by default when running `outlier` without any arguments

## [0.2.3] - 2026-01-16

### Added
- **README Documentation**: Added missing documentation for recent features
  - Observability section with Honeycomb/OpenTelemetry configuration
  - Volume testing section with `--count`, `--with-api`, and `--api-url` options

### Changed
- Updated health endpoint version example in README

## [0.2.2] - 2026-01-16

### Added
- **Volume Test `--count` Argument**: Configurable value count for volume testing
  - Run with `cargo run --example volume_test -- --count 100000` for custom counts
  - Defaults to 1,000,000 values for backwards compatibility

## [0.2.1] - 2026-01-15

### Added
- **Volume Test Script**: `examples/volume_test.rs` for benchmarking with 1 million values
  - Tests 95th and 90th percentile calculations
  - Supports both direct library tests and HTTP API endpoint tests
  - Run with `cargo run --example volume_test` (library only) or `--with-api` flag for API tests
- `reqwest` dev-dependency for HTTP API testing

### Changed
- Increased server body limit from 2MB to 100MB to support large datasets (1M+ values)
- Added `Deserialize` derive to `CalculateResponse` for API response parsing

## [0.2.0] - 2026-01-15

### Added
- **Honeycomb.io Observability**: Distributed tracing via OpenTelemetry OTLP exporter
- Telemetry module (`telemetry.rs`) for initializing tracing pipeline
- Instrumentation spans for core functions (`calculate_percentile`, `read_values_from_file`, `read_values_from_bytes`)
- Instrumentation spans for HTTP handlers (`calculate`, `calculate_file`, `health`)
- Environment variable configuration:
  - `HONEYCOMB_API_KEY`: API key for Honeycomb (falls back to console logging if unset)
  - `OTEL_SERVICE_NAME`: Service name (defaults to "outlier")

### Changed
- Tracing now enabled for both CLI and server modes
- Tokio runtime now a core dependency (required for async OTLP exporter)

## [0.1.5] - 2026-01-14

## [0.1.4] - 2026-01-14

## [0.1.3] - 2026-01-14

## [0.1.2] - 2026-01-14

## [0.1.1] - 2026-01-14

### Added
- Version flag support (`--version` / `-V`)
- CHANGELOG.md for tracking changes
- cargo-release configuration for automated version management
- RELEASING.md with comprehensive release documentation
- **API Server Mode**: Run outlier as an HTTP REST API with `--serve` flag
- **OpenAPI/Swagger Documentation**: Interactive API docs at `/docs` endpoint
- **POST /calculate**: Calculate percentiles from JSON request body
- **POST /calculate/file**: Upload and process JSON/CSV files via multipart form
- **GET /health**: Health check endpoint for monitoring
- Library module (`lib.rs`) with reusable percentile functions
- Optional server feature flag to keep CLI binary lightweight
- Axum-based web server with CORS and tracing support

### Changed
- Updated CLI description from "performance rates" to "percentiles from numerical datasets"
- Renamed project from "prate" to "outlier"
- Refactored core logic into library for reusability

## [0.1.0] - 2026-01-14

### Added
- Initial release of outlier (Percentile Calculator)
- Percentile calculation using linear interpolation
- Support for JSON file input (array of numbers)
- Support for CSV file input (single column with header)
- Support for command-line array input (comma-separated values)
- Configurable percentile value via `-p` / `--percentile` flag (default: 95)
- Comprehensive unit tests (10 test cases)
- Docker support with multi-stage build
- Makefile for build automation
- GitHub Actions CI workflow (tests, clippy, cargo audit, formatting)
- GitHub Actions CodeQL security scanning
- MIT License
- Comprehensive README with examples and documentation
- Example data files (JSON and CSV)
