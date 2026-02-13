# Build stage
FROM rust:1.92 AS builder

WORKDIR /usr/src/outlier

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy src to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release --features server
RUN rm -rf src

# Copy source code and rebuild
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release --features server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/outlier/target/release/outlier /usr/local/bin/outlier

EXPOSE 3000

# Set the entrypoint
ENTRYPOINT ["outlier"]
CMD ["--serve"]
