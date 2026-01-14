# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Version flag support (`--version` / `-V`)
- CHANGELOG.md for tracking changes
- cargo-release configuration for automated version management
- RELEASING.md with comprehensive release documentation

### Changed
- Updated CLI description from "performance rates" to "percentiles from numerical datasets"
- Renamed project from "prate" to "outlier"

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
