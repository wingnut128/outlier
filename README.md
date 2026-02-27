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
- 6 interpolation methods: linear (default), nearest_rank, lower, upper, midpoint, nearest_even
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
Method: linear
Percentile (P95): 9.55
```

### Specify Percentile

Calculate the 99th percentile:
```bash
outlier -p 99 -v 1,2,3,4,5,6,7,8,9,10
```

### Specify Interpolation Method

Use a different interpolation method:
```bash
outlier -p 40 -m nearest_rank -v 1,2,3,4,5
```

See [How Percentiles Work](#how-percentiles-work) for a description of each method.

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

## API Server Mode

outlier can run as an HTTP API server with OpenAPI/Swagger documentation.

### Starting the Server

```bash
# Start server on default port 3000
cargo run --features server -- --serve

# Or use the compiled binary
outlier --serve

# Specify a custom port
outlier --serve --port 8080
```

The server provides:
- 🚀 REST API endpoints at `http://localhost:3000`
- 📚 Interactive Swagger UI at `http://localhost:3000/docs`
- 📖 OpenAPI spec at `http://localhost:3000/api-docs/openapi.json`

### API Endpoints

#### POST /calculate
Calculate percentile from JSON array:

```bash
curl -X POST http://localhost:3000/calculate \
  -H "Content-Type: application/json" \
  -d '{
    "values": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    "percentile": 95,
    "method": "linear"
  }'
```

Response:
```json
{
  "count": 10,
  "percentile": 95.0,
  "result": 9.55,
  "method": "linear"
}
```

#### POST /calculate/file
Upload a file (JSON or CSV) for calculation:

```bash
curl -X POST http://localhost:3000/calculate/file \
  -F "file=@data.json" \
  -F "percentile=99" \
  -F "method=nearest_rank"
```

Response:
```json
{
  "count": 100,
  "percentile": 99.0,
  "result": 98.01,
  "method": "nearest_rank"
}
```

#### GET /health
Health check endpoint:

```bash
curl http://localhost:3000/health
```

Response:
```json
{
  "status": "healthy",
  "service": "outlier",
  "version": "0.5.1"
}
```

### Authentication

Authentication is optional and disabled by default. Enable it in your config file:

#### API Key Mode

```toml
[auth]
enabled = true
mode = "api_key"
```

Set keys via environment variable (recommended) or config file:
```bash
export OUTLIER_API_KEYS="key1,key2,key3"
```

```bash
curl -X POST http://localhost:3000/calculate \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{"values": [1,2,3,4,5], "percentile": 95}'
```

#### JWT/IdP Mode

Supports Auth0, Google, Okta, and any OIDC-compliant provider:

```toml
[auth]
enabled = true
mode = "jwt"

[auth.jwt]
issuer = "https://your-tenant.auth0.com/"
audience = "https://api.your-domain.com"
```

Env var overrides are available: `OUTLIER_JWT_ISSUER`, `OUTLIER_JWT_AUDIENCE`, `OUTLIER_JWT_JWKS_URL`.

```bash
curl -X POST http://localhost:3000/calculate \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <your-jwt-token>" \
  -d '{"values": [1,2,3,4,5], "percentile": 95}'
```

#### Both Mode

Accepts either `X-API-Key` or `Authorization: Bearer` headers:

```toml
[auth]
enabled = true
mode = "both"

[auth.jwt]
issuer = "https://your-tenant.auth0.com/"
audience = "https://api.your-domain.com"
```

The `/health`, `/docs`, and `/api-docs` endpoints are always accessible without authentication.

### Rate Limiting

Optional per-IP and global rate limiting, disabled by default:

```toml
[rate_limit]
enabled = true
per_ip_per_second = 10
per_ip_burst = 20
global_per_second = 100
global_burst = 200
```

When rate limited, the server returns `429 Too Many Requests` with a `Retry-After` header.

## Observability

outlier supports distributed tracing via OpenTelemetry, with built-in support for [Honeycomb.io](https://honeycomb.io).

### Configuration

Set the following environment variables to enable tracing:

| Variable | Description | Default |
|----------|-------------|---------|
| `HONEYCOMB_API_KEY` | Your Honeycomb API key | (none - falls back to console logging) |
| `OTEL_SERVICE_NAME` | Service name for traces | `outlier` |

### Example

```bash
# Enable Honeycomb tracing
export HONEYCOMB_API_KEY="your-api-key"
export OTEL_SERVICE_NAME="outlier-production"

# Start the server with tracing enabled
outlier --serve
```

When `HONEYCOMB_API_KEY` is not set, tracing output falls back to console logging.

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

### Build

```bash
docker build -t outlier:latest .
```

### Run the Server

```bash
# Start the API server (default mode)
docker run --rm -p 3000:3000 outlier:latest

# With a config file
docker run --rm -p 3000:3000 -v $(pwd)/config.example.toml:/etc/outlier/config.toml:ro \
  outlier:latest --serve --config /etc/outlier/config.toml
```

### Run CLI Mode

```bash
# Calculate percentile from CLI values
docker run --rm outlier:latest -v 1,2,3,4,5,6,7,8,9,10 -p 95

# Run with a file (mount the examples directory)
docker run --rm -v $(pwd)/examples:/data outlier:latest -f /data/sample.json -p 99
```

### Docker Compose

For local development with a config file mounted:

```bash
docker compose up
```

The server will be available at `http://localhost:3000`. See `docker-compose.yml` for configuration.

## Testing

The project includes comprehensive unit tests covering:
- Various percentile values (P0, P50, P95, P99, P100)
- Per-method algorithm correctness for all 6 interpolation methods
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

### Volume Testing

A volume test script is included for benchmarking with large datasets:

```bash
# Run with default 1 million values
cargo run --example volume_test

# Run with custom value count
cargo run --example volume_test -- --count 500000

# Include API endpoint tests (start server first)
cargo run --example volume_test -- --with-api

# Use custom API URL
cargo run --example volume_test -- --with-api --api-url http://localhost:8080
```

The volume test measures:
- Value generation time
- Percentile calculation throughput (values/sec)
- Library vs API result consistency

## Command-Line Options

- `-p, --percentile <VALUE>`: Percentile to calculate (0-100). Default: 95
- `-m, --method <METHOD>`: Interpolation method. Values: `linear`, `nearest_rank`, `lower`, `upper`, `midpoint`, `nearest_even`. Default: `linear`
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

For a given percentile P, values are sorted in ascending order and the fractional index is calculated as `(P/100) × (N-1)`. The interpolation method determines how that index maps to a result value:

| Method | Description |
|--------|-------------|
| `linear` | Linearly interpolate between the two adjacent values (default) |
| `nearest_rank` | Round the index to the nearest integer |
| `lower` | Always round the index down (floor) |
| `upper` | Always round the index up (ceil) |
| `midpoint` | Average the floor and ceil values |
| `nearest_even` | Round half to even index (banker's rounding) |

**Example — P95 of [1,2,3,4,5,6,7,8,9,10] using `linear`:**
- Index = 0.95 × 9 = 8.55
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
