# Railspan developer commands

default:
    @just --list

# Build all Rust crates
build:
    cargo build --workspace

# Test Rust + Ruby
test: test-rust test-gem

test-rust:
    cargo test --workspace

test-gem:
    cd gem/railspan && bundle install --quiet && bundle exec rake test

# Run agent on default port
serve:
    cargo run -p railspan-cli -- serve --addr 127.0.0.1:7421

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Boot dummy Rails (requires bundle install in examples/dummy_rails)
dummy-rails:
    cd examples/dummy_rails && bundle exec rails server -p 3000
