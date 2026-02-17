VERSION = $(shell cat ./Cargo.toml | grep ^version | awk -F ' = ' '{ print $$2 }' | tr -d '"')

# Build the binary
.PHONY: build
build:
	cargo build --release

# Build the binary
.PHONY: all
all:
	cargo build --all-features --release

# Build documentation for the library
.PHONY: doc
doc:
	cargo fmt --check
	cargo doc --no-deps

# Run all tests (no coverage)
.PHONY: test
test:
	cargo test --all-features

# Run all tests (no coverage)
.PHONY: test-logs
test-logs:
	cargo test --all-features -- --nocapture

# Clean up
.PHONY: clean
clean:
	cargo clean

# ==== Directives for developers ====

# Print the version of the package
.PHONY: version
version:
	@echo v$(VERSION)

# Run only unit tests (shorthand for developers)
.PHONY: unit-test
unit-test:
	cargo test --all-targets

# Run only integration tests (shorthand for developers)
.PHONY: integration-test
integration-test:
	cargo test --test '*'

# ==== Helper directives ====

# Format codebase
.PHONY: format
format:
	cargo fmt
	dprint fmt

# Lint codebase
.PHONY: lint
lint:
	dprint check
	cargo check --all-targets --all-features
	cargo fmt --all --check
	cargo clippy --all-targets --all-features -- -D warnings
