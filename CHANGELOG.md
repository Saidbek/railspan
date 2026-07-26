# Changelog

## 0.1.1 — 2026-07-18

### Added (Phase 5 — production hardening)
- Query API auth via Bearer `RAILSPAN_UI_TOKEN` (defaults to API key); UI Auth button
- Adaptive sampling advice on ingest (`advice.sample_rate`); gem adopts lower rates
- Gem head sampling for requests/jobs from `sample_rate`
- Stricter cardinality guards: max spans/batch, body size, attribute/event caps
- Structured logs: `--log-format json` / `RAILSPAN_LOG_FORMAT`
- Overhead microbench: `scripts/bench_overhead.sh` + `docs/OVERHEAD.md`
- Soak runbook and security checklist under `docs/`

## 0.1.0 — 2026-07-11

### Added
- Ruby gem: Rack, controller, SQL, view, cache, Net::HTTP, ActiveJob, Sidekiq
- SQL normalizer + PII scrubber + stdout/HTTP exporters
- Rust `railspan serve`: ingest, SQLite store, query API, embedded UI
- N+1 detection with UI badges and list
- Deploy markers API + UI
- Server-side sampling, retention worker, cardinality truncation
- Dummy Rails dogfood app
- CI, Docker, user guide
