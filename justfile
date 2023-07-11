# List recipes
list:
  @just --list --unsorted

#############################################
# Development:

# Good to run before committing changes
all: test format clippy

# cargo check
check:
  cargo check

# Run tests
test *args='':
  cargo test {{args}}

# Run tests with --nocapture
test-nocapture *args='':
  cargo test -- --nocapture {{args}}

# Format source code
format:
  cargo fmt

# Run clippy
clippy:
  cargo clippy --all-targets -- -D warnings

# Build
build *args='--release':
  cargo build {{ args }}

# Install
install: build
  cargo install --path .

# (cargo install cargo-modules)
# Show module tree
tree:
  cargo modules generate tree --with-types --with-traits --with-fns

# (cargo install --locked cargo-outdated)
# Show outdated dependencies
outdated:
  cargo outdated --root-deps-only

# (cargo install --locked cargo-udeps)
# Find unused dependencies
udeps:
  cargo +nightly udeps

# cargo update
update:
  cargo update

#############################################
# Exercise the program:

# Run program
run *args='':
  cargo run -- {{args}}

# Check configuration
check-config *args='':
  cargo run -- check-config {{args}}
