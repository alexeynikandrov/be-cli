# Makefile for the Rust implementation of `be` (rust.src).
# All build and verification is funneled through these targets.

.PHONY: all build run test lint fmt fmt-check clippy check clean

# Default: full verification.
all: check

# Compile the project.
build:
	cargo build

# Run the editor. Pass arguments via ARGS, e.g. `make run ARGS="notes.txt"`.
run:
	cargo run -- $(ARGS)

# Run the test suite.
test:
	cargo test

# Lint: formatting check + clippy with warnings denied.
lint: fmt-check clippy

# Apply rustfmt formatting.
fmt:
	cargo fmt

# Verify formatting without modifying files.
fmt-check:
	cargo fmt --check

# Static analysis; fail on any warning.
clippy:
	cargo clippy --all-targets -- -D warnings

# Full gate before commit/merge: build + test + lint.
check: build test lint

# Remove build artifacts.
clean:
	cargo clean
