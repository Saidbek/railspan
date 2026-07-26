default:
    @just --list

build:
    cargo build --workspace

release:
    cargo build -p railspan-cli --release

test: test-rust test-gem

test-rust:
    cargo test --workspace

test-gem:
    cd gem/railspan && bundle install --quiet && bundle exec rake test

serve:
    cargo run -p railspan-cli -- serve --addr 127.0.0.1:7421 --data-dir ./data --source-root ./examples/dummy_rails

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

bench:
    ruby scripts/bench_overhead.rb 20000

# Production-like mixed traffic against dummy_rails + railspan serve
# Args: duration concurrency e.g. `just load-test 60 20`
load-test duration="45" concurrency="12":
    ruby scripts/load_test.rb --duration {{duration}} --concurrency {{concurrency}}

# Aggressive flood (more N+1, higher pressure on adaptive sampling)
load-flood duration="30" concurrency="40":
    ruby scripts/load_test.rb --flood --duration {{duration}} --concurrency {{concurrency}}

dummy:
    cd examples/dummy_rails && RAILSPAN_EXPORTER=http RAILSPAN_ENDPOINT=http://127.0.0.1:7421 bundle exec rails server -p 3000
