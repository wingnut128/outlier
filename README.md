# prate - Performance Rate Calculator

A fast and efficient command-line tool for calculating percentiles (performance rates) from input values.

## Features

- Calculate any percentile (P50, P95, P99, etc.) from a dataset
- Multiple input methods:
  - JSON files
  - CSV files
  - Direct CLI values
- Comprehensive unit tests
- Docker support
- Easy build with Makefile

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
docker build -t prate:latest .
```

## Usage

### Basic Usage

Calculate the 95th percentile (default) from CLI values:
```bash
prate -v 1,2,3,4,5,6,7,8,9,10
```

### Specify Percentile

Calculate the 99th percentile:
```bash
prate -p 99 -v 1,2,3,4,5,6,7,8,9,10
```

### From JSON File

```bash
prate -p 95 -f examples/sample.json
```

Example JSON format:
```json
[1.5, 2.3, 4.7, 8.1, 12.5, 15.9, 23.4, 34.6, 45.2, 67.8]
```

### From CSV File

```bash
prate -p 99 -f examples/sample.csv
```

Example CSV format:
```csv
value
1.5
2.3
4.7
8.1
```

### Help

```bash
prate --help
```

## Building

### Using Makefile

```bash
# Build debug version
make build

# Build release version
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
# Build
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
docker build -t prate:latest .

# Run with CLI values
docker run --rm prate:latest -v 1,2,3,4,5,6,7,8,9,10 -p 95

# Run with a file (mount the file)
docker run --rm -v $(pwd)/examples:/data prate:latest -f /data/sample.json -p 99
```

## Testing

The project includes comprehensive unit tests:

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
- `-f, --file <PATH>`: Input file (JSON or CSV format)
- `-v, --values <VALUES>`: Comma-separated values from command line
- `-h, --help`: Print help information

## Examples

```bash
# Calculate P50 (median) from CLI values
prate -p 50 -v 10,20,30,40,50

# Calculate P99 from JSON file
prate -p 99 -f data.json

# Calculate P95 from CSV file
prate -p 95 -f data.csv

# Default P95 calculation
prate -v 100,200,300,400,500,600,700,800,900,1000
```

## License

MIT
