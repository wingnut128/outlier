# outlier - Percentile Calculator

[![CI](https://github.com/wingnut128/outlier/actions/workflows/ci.yml/badge.svg)](https://github.com/wingnut128/outlier/actions/workflows/ci.yml)
[![CodeQL](https://github.com/wingnut128/outlier/actions/workflows/codeql.yml/badge.svg)](https://github.com/wingnut128/outlier/actions/workflows/codeql.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A fast and efficient command-line tool for calculating percentiles from numerical datasets. Percentiles are statistical measures that indicate the value below which a given percentage of observations fall in a distribution. Commonly used for analyzing performance metrics, response times, system latencies, and other data distributions.

> 🤖 **Generated with [Claude Code](https://claude.com/claude-code)**
> This project was created using Claude Code, an AI-powered CLI tool for software development.

## Features

- Calculate any percentile (P50/median, P95, P99, etc.) from a dataset
- Multiple input methods:
  - JSON files (array of numbers)
  - CSV files (single column of values)
  - Direct CLI values (comma-separated)
- Linear interpolation for accurate percentile calculation
- Comprehensive unit tests with 100% coverage
- Docker support for containerized environments
- Makefile for convenient build automation

## Installation

### From Source

```bash
# Build and install
make install

# Or using cargo directly
cargo install --path .
```

### Using Docker

```bash
# Build the Docker image
make docker-build

# Or using docker directly
docker build -t outlier:latest .
```

## Usage

### Basic Usage

Calculate the 95th percentile (default) from CLI values:
```bash
outlier -v 1,2,3,4,5,6,7,8,9,10
```

Output:
```
Number of values: 10
Percentile (P95): 9.55
```

### Specify Percentile

Calculate the 99th percentile:
```bash
outlier -p 99 -v 1,2,3,4,5,6,7,8,9,10
```

### From JSON File

```bash
outlier -p 95 -f examples/sample.json
```

Example JSON format (array of numbers):
```json
[1.5, 2.3, 4.7, 8.1, 12.5, 15.9, 23.4, 34.6, 45.2, 67.8]
```

### From CSV File

```bash
outlier -p 99 -f examples/sample.csv
```

Example CSV format (header row "value", one value per line):
```csv
value
1.5
2.3
4.7
8.1
```

### Help

```bash
outlier --help
```

## Building

### Using Makefile

```bash
# Build debug version
make build

# Build release version (optimized)
make release

# Run tests
make test

# Clean build artifacts
make clean

# View all available commands
make help
```

### Using Cargo

```bash
# Build release version
cargo build --release

# Run tests
cargo test

# Run the application
cargo run -- -v 1,2,3,4,5
```

## Docker Usage

### Build and Run

```bash
# Build the image
docker build -t outlier:latest .

# Run with CLI values
docker run --rm outlier:latest -v 1,2,3,4,5,6,7,8,9,10 -p 95

# Run with a file (mount the examples directory)
docker run --rm -v $(pwd)/examples:/data outlier:latest -f /data/sample.json -p 99
```

## Testing

The project includes comprehensive unit tests covering:
- Various percentile values (P0, P50, P95, P99, P100)
- Edge cases (empty datasets, single values, duplicates)
- Unsorted input handling
- Large datasets (1000+ values)

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_calculate_percentile_95th
```

## Command-Line Options

- `-p, --percentile <VALUE>`: Percentile to calculate (0-100). Default: 95
- `-f, --file <PATH>`: Input file path (JSON or CSV format)
- `-v, --values <VALUES>`: Comma-separated numerical values
- `-h, --help`: Print help information

## Examples

```bash
# Calculate P50 (median) from CLI values
outlier -p 50 -v 10,20,30,40,50

# Calculate P99 from JSON file
outlier -p 99 -f data.json

# Calculate P95 from CSV file
outlier -p 95 -f data.csv

# Calculate default P95 from response times
outlier -v 100,200,300,400,500,600,700,800,900,1000
```

## How Percentiles Work

The tool uses linear interpolation to calculate percentiles accurately. For a given percentile P:

1. Values are sorted in ascending order
2. The position is calculated as: `(P/100) × (N-1)` where N is the count of values
3. If the position falls between two values, linear interpolation is used to determine the result

For example, P95 of [1,2,3,4,5,6,7,8,9,10]:
- Position = 0.95 × 9 = 8.55
- Result = linear interpolation between values at index 8 (9) and index 9 (10)
- P95 = 9.55

## Use Cases

- **Performance Analysis**: Analyze API response times, database query latencies
- **System Monitoring**: Calculate resource usage percentiles (CPU, memory, disk I/O)
- **SLA Compliance**: Verify service level agreements (e.g., 95% of requests < 200ms)
- **Data Analysis**: General statistical analysis of any numerical distribution
- **Benchmarking**: Evaluate system performance under various conditions

## Releasing

This project uses [cargo-release](https://github.com/crate-ci/cargo-release) for version management. To create a new release:

```bash
# Preview the release (dry run is default)
cargo release patch    # 0.1.0 → 0.1.1
cargo release minor    # 0.1.0 → 0.2.0
cargo release major    # 0.1.0 → 1.0.0

# Execute the release
cargo release patch --execute
```

See [RELEASING.md](RELEASING.md) for detailed instructions.

## License

MIT License - see [LICENSE](LICENSE) file for details.
