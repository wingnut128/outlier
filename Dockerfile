# Build stage
FROM rust:1.92 AS builder

WORKDIR /usr/src/prate

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/prate/target/release/prate /usr/local/bin/prate

# Set the entrypoint
ENTRYPOINT ["prate"]
CMD ["--help"]
