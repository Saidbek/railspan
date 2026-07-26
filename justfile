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

# ── Local development (one command) ─────────────────────────────────────────
# API only on :7421 (no production Vue embed) + Vite HMR on :5173.
# Does NOT run `npm run build` / does not refresh static/assets.
# Open http://127.0.0.1:5173 — Ctrl+C stops both.
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(cd "{{justfile_directory()}}" && pwd)"
    cd "$ROOT"
    DEV_UI="http://127.0.0.1:5173"

    if [[ ! -d ui/node_modules ]]; then
      echo "→ npm install (first run)"
      (cd ui && npm install)
    fi

    echo "→ starting API (dev mode: no embedded production UI)"
    cargo run -q -p railspan-cli -- serve \
      --addr 127.0.0.1:7421 \
      --data-dir ./data \
      --source-root ./examples/dummy_rails \
      --dev-ui-url "${DEV_UI}" &
    API_PID=$!

    cleanup() {
      echo
      echo "→ stopping API (pid ${API_PID})"
      kill "${API_PID}" 2>/dev/null || true
      wait "${API_PID}" 2>/dev/null || true
    }
    trap cleanup EXIT INT TERM

    for _ in $(seq 1 60); do
      if curl -sf http://127.0.0.1:7421/healthz >/dev/null 2>&1; then
        break
      fi
      if ! kill -0 "${API_PID}" 2>/dev/null; then
        echo "error: API process exited early" >&2
        exit 1
      fi
      sleep 0.25
    done
    if ! curl -sf http://127.0.0.1:7421/healthz >/dev/null 2>&1; then
      echo "error: API did not become ready on :7421" >&2
      exit 1
    fi

    echo
    echo "  Dev UI  →  ${DEV_UI}   ← open this (Vite, hot reload)"
    echo "  API     →  http://127.0.0.1:7421  (ingest/query only; / redirects to Vite)"
    echo "  Prod UI →  just serve   (builds static/ + embeds on :7421)"
    echo
    cd ui && npm run dev

# Production-like: npm build Vue → static/, embed in binary, single process on :7421
serve: ui-build
    cargo build -p railspan-cli
    cargo run -p railspan-cli -- serve --addr 127.0.0.1:7421 --data-dir ./data --source-root ./examples/dummy_rails

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

bench:
    ruby scripts/bench_overhead.rb 20000

# Vue helpers (usually you only need `just dev`)
ui-install:
    cd ui && npm install

# Alias — same as `just dev`
ui-dev: dev

ui-build:
    cd ui && npm run build

# Production-like mixed traffic against dummy_rails + railspan serve
# Args: duration concurrency e.g. `just load-test 60 20`
load-test duration="45" concurrency="12":
    ruby scripts/load_test.rb --duration {{duration}} --concurrency {{concurrency}}

# Aggressive flood (more N+1, higher pressure on adaptive sampling)
load-flood duration="30" concurrency="40":
    ruby scripts/load_test.rb --flood --duration {{duration}} --concurrency {{concurrency}}

dummy:
    cd examples/dummy_rails && RAILSPAN_EXPORTER=http RAILSPAN_ENDPOINT=http://127.0.0.1:7421 bundle exec rails server -p 3000
