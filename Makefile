.PHONY: build test clean run dev serve-test install help docker-build docker-run release

BINARY_NAME=outlier
DOCKER_IMAGE=outlier:latest

help:
	@echo "Available targets:"
	@echo "  build         - Build the project in debug mode"
	@echo "  release       - Build the project in release mode"
	@echo "  test          - Run all tests"
	@echo "  clean         - Clean build artifacts"
	@echo "  run           - Run the CLI"
	@echo "  dev           - Run the API server with dev config"
	@echo "  serve-test    - Run the API server with auth enabled (key: dev-test-key)"
	@echo "  install       - Install the binary to cargo bin"
	@echo "  docker-build  - Build Docker image"
	@echo "  docker-run    - Run Docker container"

build:
	cargo build --features server

release:
	cargo build --release --features server

test:
	cargo test

clean:
	cargo clean

run:
	cargo run --features server

dev:
	cargo run --features server -- --serve --config config.development.toml

serve-test:
	cargo run --features server -- --serve --config config.test.toml

install:
	cargo install --features server --path .

docker-build:
	docker build -t $(DOCKER_IMAGE) .

docker-run:
	docker run --rm $(DOCKER_IMAGE) --help
