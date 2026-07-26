#!/usr/bin/env bash
# Overhead harness entrypoint (Phase 5).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== Railspan overhead microbench =="
ruby scripts/bench_overhead.rb "${1:-50000}"

if [[ "${RAILSPAN_BENCH_FULL:-}" == "1" ]]; then
  echo
  echo "== Optional full path: ensure dummy_rails + railspan serve are running =="
  echo "  cargo run -p railspan-cli -- serve --data-dir ./data &"
  echo "  cd examples/dummy_rails && RAILSPAN_EXPORTER=http bin/rails s -p 3000 &"
  echo "  ab -n 2000 -c 10 http://127.0.0.1:3000/users"
  echo "Compare RAILSPAN_ENABLED=0 vs 1; target < ~2% p95 regression."
fi
