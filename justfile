image_name := 'mbari/o2d'
# version read from Cargo.toml

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

# Clean
clean:
  cargo clean

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
# Exercise the program (via cargo):

# Run program
run *args='':
  cargo run -- {{args}}

# Check configuration
check-config *args='':
  cargo run -- check-config {{args}}

# Get all platforms
get-platforms *args:
  cargo run -- get-platforms {{args}}

# Get platform information
get-platform platform='5d5b2ea653a65f9ec656d872':
  cargo run -- get-platform {{platform}}

# Get platform positions
get-positions platform='54065b5560d0e168c88d4043':
  cargo run -- get-positions {{platform}}

# Add platforms to be dispatched
add-dispatched *args='':
  cargo run -- add-dispatched {{args}}

# Run dispatch
dispatch *args='':
  cargo run -- dispatch {{args}}

# Run server and dispatch
serve *args:
  cargo run -- serve {{args}}

# Run server but no dispatch
serve-no-dispatch *args:
  cargo run -- serve --no-dispatch {{args}}

#############################################
# docker recipes:

# Dockerize
dockerize *args='':
  docker build -f docker/Dockerfile -t "mbari/o2d:2.0.0" {{args}} .

#############################################
# Exercise dockerized program:

# docker run
docker-run *args='':
    docker run --name=o2d -it --rm \
           -e RUST_LOG=info \
           -e RUST_BACKTRACE=full \
           -v $(pwd):/public \
           -p 3033:3033  \
           mbari/o2d:2.0.0 {{args}}

# Push image to Docker Hub, including x.y.z, x.y, x, and 'latest' tags
docker-push-image:
  #!/usr/bin/env bash
  version=$(cat Cargo.toml | grep version | head -1 | cut -d\" -f2)
  mayor_minor=$(echo $version | cut -d. -f1,2)
  mayor=$(echo $version}| cut -d. -f1)
  echo "    version='${version}'"
  echo "mayor_minor='${mayor_minor}'"
  echo "      mayor='${mayor}'"
  docker push "{{image_name}}:$version"
  just docker-tag-push-image $version "$mayor_minor"
  just docker-tag-push-image $version "$mayor"
  just docker-tag-push-image $version latest

# tag and push image
docker-tag-push-image version tag:
  docker tag "{{image_name}}:{{version}}" "{{image_name}}:{{tag}}"
  docker push "{{image_name}}:{{tag}}"

#############################################
# With local server running:

# Get platforms via REST API against TrackingDB
rest-trackdb-get-platforms:
  curlie get http://localhost:3033/trackdb/platforms

# Get platform information via REST API against TrackingDB
rest-trackdb-get-platform platform='5d5b2ea653a65f9ec656d872':
  curlie get http://localhost:3033/trackdb/platforms/{{platform}}

# Get platform positions via REST API against TrackingDB
rest-trackdb-get-positions platform='54065b5560d0e168c88d4043' lastNumberOfFixes='2':
  curlie get http://localhost:3033/trackdb/platforms/{{platform}}/positions?lastNumberOfFixes={{lastNumberOfFixes}}

# Get dispatched platforms via REST API
rest-dispatched-get-platforms:
  curlie get http://localhost:3033/dispatched/platforms

# Get dispatched platform information via REST API
rest-dispatched-get-platform platform='5d5b2ea653a65f9ec656d872':
  curlie get http://localhost:3033/dispatched/platforms/{{platform}}

# Add dispatched platform via REST API
rest-dispatched-add-platforms platformIds='["001", "002", "003"]':
  curlie post http://localhost:3033/dispatched/platforms \
    platformIds:='{{platformIds}}'

# Delete dispatched platform via REST API
rest-dispatched-delete-platform platform:
  curlie delete http://localhost:3033/dispatched/platforms/{{platform}}

# Delete dispatched platforms via REST API
rest-dispatched-delete-platforms:
  just rest-dispatched-delete-platform 002
  just rest-dispatched-delete-platform 001
  just rest-dispatched-delete-platform 003
