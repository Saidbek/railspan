# Railspan developer commands

default:
    @just --list

build:
    cargo build --workspace

test: test-rust test-gem

test-rust:
    cargo test --workspace

test-gem:
    cd gem/railspan && bundle install --quiet && bundle exec rake test

serve:
    cargo run -p railspan-cli -- serve --addr 127.0.0.1:7421 --data-dir ./data

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

dummy-rails:
    cd examples/dummy_rails && RAILSPAN_EXPORTER=http RAILSPAN_ENDPOINT=http://127.0.0.1:7421 bundle exec rails server -p 3000
