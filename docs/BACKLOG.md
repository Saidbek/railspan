# Backlog — Epics & Stories (Jira-ready)

Use this document to create **Epics** and **child Stories** in Jira/Linear/GitHub.

**Conventions**

- **ID:** `E#` epic, `E#-S#` story  
- **Points:** Fibonacci suggestion (1, 2, 3, 5, 8, 13)  
- **Priority:** P0 (MVP), P1, P2, P3  
- Each story has **Acceptance Criteria (AC)**  
- Labels: `gem`, `agent`, `server`, `ui`, `docs`, `infra`, `perf`, `security`

---

## Epic index

| Epic | Title | Priority | Phase |
|------|-------|----------|-------|
| E0 | Monorepo foundation & tooling | P0 | 0 |
| E1 | Ruby gem — core instrumentation | P0 | 1 |
| E2 | Rust agent — ingest & processing | P0 | 2 |
| E3 | Server, storage & vertical-slice UI | P0 | 3 |
| E4 | Rails-depth: N+1, jobs, deploys | P1 | 4 |
| E5 | Production hardening | P1 | 5 |
| E6 | Packaging, release & docs | P1 | 6 |
| E7 | Scale & OpenTelemetry interop | P2 | 7 |
| E8 | DX, examples & dogfooding | P0 | ongoing |

---

# E0 — Monorepo foundation & tooling

**Goal:** Buildable workspace, CI, doc structure, dummy app shell.  
**Exit:** Empty `railspan serve` stub runs; gem installs into dummy Rails.

### E0-S1 — Create Cargo workspace skeleton
- **Points:** 3 · **Labels:** `infra` · **P0**
- **Description:** Initialize `crates/{railspan-cli,railspan-agent,railspan-server,railspan-protocol}` with hello builds.
- **AC:**
  - [ ] Root `Cargo.toml` workspace members listed
  - [ ] `cargo build --workspace` succeeds
  - [ ] `cargo test --workspace` succeeds (even if no real tests yet)

### E0-S2 — Create Ruby gem skeleton
- **Points:** 3 · **Labels:** `gem` · **P0**
- **Description:** `gem/railspan` with version, Railtie stub, config module.
- **AC:**
  - [ ] Gem builds (`gem build`)
  - [ ] Bundler can path-load gem in dummy app
  - [ ] `Railspan::VERSION` defined

### E0-S3 — Dummy Rails application
- **Points:** 5 · **Labels:** `infra`, `gem` · **P0**
- **Description:** `examples/dummy_rails` with a few endpoints (including intentional N+1 seed data later).
- **AC:**
  - [ ] App boots with Rails 7.1+ / 8.x
  - [ ] Routes: health, simple show, list with associations
  - [ ] SQLite or PG documented

### E0-S4 — CI pipeline
- **Points:** 3 · **Labels:** `infra` · **P0**
- **Description:** GitHub Actions (or preferred CI) for Rust + Ruby.
- **AC:**
  - [ ] PR runs `cargo fmt --check`, `clippy`, `test`
  - [ ] PR runs gem tests
  - [ ] Status badge optional in README

### E0-S5 — ADR template + open questions log
- **Points:** 1 · **Labels:** `docs` · **P0**
- **AC:**
  - [ ] `docs/adrs/0000-template.md` exists
  - [ ] License + name decisions recorded or listed as open

### E0-S6 — Docker compose stub
- **Points:** 2 · **Labels:** `infra` · **P1**
- **AC:**
  - [ ] `docker/docker-compose.yml` builds placeholder service
  - [ ] Documented in README

---

# E1 — Ruby gem — core instrumentation

**Goal:** Correct span trees; export batches to agent (or stdout).  
**Exit:** Dummy request → nested spans with SQL + controller + view.

### E1-S1 — Config DSL & enable flags
- **Points:** 2 · **Labels:** `gem` · **P0**
- **AC:**
  - [ ] `Railspan.configure` block works
  - [ ] ENV overrides documented
  - [ ] `enabled=false` disables all hooks

### E1-S2 — Trace/span ID generation & context
- **Points:** 5 · **Labels:** `gem` · **P0**
- **Description:** Thread-local (and fiber-safe strategy) context stack.
- **AC:**
  - [ ] Nested spans get parent ids
  - [ ] Context cleared after request
  - [ ] Unit tests for stack push/pop

### E1-S3 — Rack middleware root span
- **Points:** 3 · **Labels:** `gem` · **P0**
- **AC:**
  - [ ] Root span name/kind `http.server`
  - [ ] Method, path/route, status attributes
  - [ ] Errors set status error

### E1-S4 — Action Controller instrumentation
- **Points:** 3 · **Labels:** `gem` · **P0**
- **AC:**
  - [ ] Span resource `Controller#action`
  - [ ] Subscribes to `process_action.action_controller`

### E1-S5 — ActiveRecord SQL spans
- **Points:** 5 · **Labels:** `gem` · **P0**
- **AC:**
  - [ ] Span per SQL (with sampling hook later)
  - [ ] `db.statement` normalized fingerprint
  - [ ] Duration reflects real query time

### E1-S6 — SQL normalizer
- **Points:** 5 · **Labels:** `gem`, `security` · **P0**
- **AC:**
  - [ ] Literals → `?`
  - [ ] IN-lists collapsed
  - [ ] Truncation above max length
  - [ ] Golden tests for common SQL

### E1-S7 — Action View template/partial spans
- **Points:** 3 · **Labels:** `gem` · **P0**
- **AC:**
  - [ ] Template and partial events create spans
  - [ ] Nested under controller

### E1-S8 — Batch exporter (HTTP)
- **Points:** 5 · **Labels:** `gem` · **P0**
- **AC:**
  - [ ] Flushes on size and interval
  - [ ] Bounded queue; drops with counter when full
  - [ ] Never raises into request (fail-open)
  - [ ] Gzip optional

### E1-S9 — Stdout/JSON debug exporter
- **Points:** 2 · **Labels:** `gem`, `dx` · **P0**
- **AC:**
  - [ ] `exporter = :stdout` for local debug
  - [ ] One JSON object per span or per batch documented

### E1-S10 — Cache instrumentation
- **Points:** 3 · **Labels:** `gem` · **P1**
- **AC:**
  - [ ] read/write/fetch spans with hit/miss when available

### E1-S11 — Outbound HTTP instrumentation
- **Points:** 5 · **Labels:** `gem` · **P1**
- **AC:**
  - [ ] Net::HTTP spans
  - [ ] Faraday adapter or subscriber if feasible
  - [ ] Host + method + status attributes (no secrets)

### E1-S12 — Manual tracing API
- **Points:** 2 · **Labels:** `gem` · **P1**
- **AC:**
  - [ ] `Railspan.trace("name") { ... }` helper
  - [ ] Documented for custom code

### E1-S13 — PII scrubber (gem-side)
- **Points:** 3 · **Labels:** `gem`, `security` · **P0**
- **AC:**
  - [ ] Default key denylist
  - [ ] Configurable additional keys
  - [ ] Applied before export

---

# E2 — Rust agent — ingest & processing

**Goal:** Reliable ingest path with sampling and normalization.  
**Exit:** Agent accepts gem traffic under load; health + drop metrics.

### E2-S1 — HTTP ingest endpoint
- **Points:** 5 · **Labels:** `agent` · **P0**
- **AC:**
  - [ ] `POST /v1/traces` JSON works
  - [ ] Msgpack works
  - [ ] Auth via bearer API key (shared secret MVP OK)

### E2-S2 — Protocol types shared crate
- **Points:** 3 · **Labels:** `agent`, `server` · **P0**
- **AC:**
  - [ ] `railspan-protocol` defines envelope + span structs
  - [ ] Version field validated

### E2-S3 — Bounded queue & workers
- **Points:** 5 · **Labels:** `agent` · **P0**
- **AC:**
  - [ ] Configurable queue capacity
  - [ ] Backpressure returns 429 when full
  - [ ] Workers process without deadlock

### E2-S4 — Sampling engine
- **Points:** 5 · **Labels:** `agent` · **P0**
- **AC:**
  - [ ] Always keep errors
  - [ ] Keep slow roots
  - [ ] Probabilistic keep otherwise
  - [ ] Drop health routes (config)

### E2-S5 — Agent-side SQL normalize + scrub
- **Points:** 3 · **Labels:** `agent`, `security` · **P0**
- **AC:**
  - [ ] Defense in depth if gem missed scrub
  - [ ] Unit tests

### E2-S6 — In-memory RED aggregation
- **Points:** 8 · **Labels:** `agent` · **P0**
- **AC:**
  - [ ] Per-endpoint histograms updated for roots
  - [ ] Flush rollups to server on interval
  - [ ] Percentile math validated with fixtures

### E2-S7 — Forwarder to server
- **Points:** 3 · **Labels:** `agent` · **P0**
- **AC:**
  - [ ] Forwards kept traces
  - [ ] Retries with backoff
  - [ ] Local-only mode writes directly if combined process

### E2-S8 — Health & internal metrics
- **Points:** 2 · **Labels:** `agent` · **P0**
- **AC:**
  - [ ] `GET /healthz`
  - [ ] Counters: received, dropped, exported

### E2-S9 — Combined process mode
- **Points:** 5 · **Labels:** `agent`, `server`, `cli` · **P0**
- **AC:**
  - [ ] `railspan serve` runs ingest+store+UI entrypoints
  - [ ] Single-binary DX documented

---

# E3 — Server, storage & vertical-slice UI

**Goal:** Persist and visualize endpoints + waterfalls.  
**Exit:** 15-minute happy path on laptop.

### E3-S1 — Project & API key management
- **Points:** 5 · **Labels:** `server`, `security` · **P0**
- **AC:**
  - [ ] Create project returns API key once
  - [ ] Keys stored hashed
  - [ ] Ingest rejects bad keys

### E3-S2 — SQLite schema & migrations
- **Points:** 5 · **Labels:** `server` · **P0**
- **AC:**
  - [ ] Tables per DATA_MODEL.md
  - [ ] Migrations run on boot
  - [ ] Indexes for top queries

### E3-S3 — Trace & span write path
- **Points:** 5 · **Labels:** `server` · **P0**
- **AC:**
  - [ ] Batch insert spans
  - [ ] Upsert/insert traces
  - [ ] Idempotency or accept duplicates safely

### E3-S4 — Metrics write path
- **Points:** 5 · **Labels:** `server` · **P0**
- **AC:**
  - [ ] Bucketed endpoint metrics stored
  - [ ] Merge histograms for same bucket

### E3-S5 — Query API: list endpoints
- **Points:** 5 · **Labels:** `server` · **P0**
- **AC:**
  - [ ] Filter by time range
  - [ ] Sort by p95/count/errors
  - [ ] JSON shape stable

### E3-S6 — Query API: search traces
- **Points:** 5 · **Labels:** `server` · **P0**
- **AC:**
  - [ ] Filter resource, min duration, errors only
  - [ ] Pagination

### E3-S7 — Query API: trace detail
- **Points:** 3 · **Labels:** `server` · **P0**
- **AC:**
  - [ ] Returns all spans for trace_id ordered by start
  - [ ] 404 if missing

### E3-S8 — UI scaffold
- **Points:** 5 · **Labels:** `ui` · **P0**
- **AC:**
  - [ ] Build pipeline produces static assets
  - [ ] Server serves UI
  - [ ] Basic layout + nav

### E3-S9 — UI: endpoints table
- **Points:** 5 · **Labels:** `ui` · **P0**
- **AC:**
  - [ ] Shows count, error rate, p50/p95/p99
  - [ ] Click → filtered traces

### E3-S10 — UI: trace waterfall
- **Points:** 8 · **Labels:** `ui` · **P0**
- **AC:**
  - [ ] Nested timeline visualization
  - [ ] SQL and view spans distinguishable
  - [ ] Duration labels readable

### E3-S11 — Empty states & connect instructions
- **Points:** 2 · **Labels:** `ui`, `docs` · **P0**
- **AC:**
  - [ ] No-data screen shows gem install snippet

### E3-S12 — Integration test: gem → serve → API
- **Points:** 5 · **Labels:** `infra` · **P0**
- **AC:**
  - [ ] Automated e2e smoke hits dummy endpoint and finds trace via API

---

# E4 — Rails-depth: N+1, jobs, deploys

**Goal:** Product differentiation beyond generic tracing.

### E4-S1 — N+1 detection engine
- **Points:** 5 · **Labels:** `server`, `agent` · **P1**
- **AC:**
  - [ ] Detect threshold repeats per fingerprint per trace
  - [ ] Persist `n_plus_one_events`
  - [ ] Configurable threshold

### E4-S2 — UI: N+1 badges & list
- **Points:** 3 · **Labels:** `ui` · **P1**
- **AC:**
  - [ ] Badge on endpoints and traces
  - [ ] Dedicated list page with fingerprint + count

### E4-S3 — ActiveJob instrumentation
- **Points:** 5 · **Labels:** `gem` · **P1**
- **AC:**
  - [ ] Perform span with job class + queue
  - [ ] Enqueue span optional
  - [ ] Errors recorded

### E4-S4 — Sidekiq middleware
- **Points:** 5 · **Labels:** `gem` · **P1**
- **AC:**
  - [ ] Server middleware creates root job span
  - [ ] Client middleware optional link
  - [ ] Works on open-source Sidekiq

### E4-S5 — UI: jobs dashboard
- **Points:** 5 · **Labels:** `ui` · **P1**
- **AC:**
  - [ ] Job class metrics (count, p95, errors)
  - [ ] Drill into job traces

### E4-S6 — Deploy markers API
- **Points:** 3 · **Labels:** `server` · **P1**
- **AC:**
  - [ ] POST deploy with git sha / version
  - [ ] List deploys in range

### E4-S7 — UI: deploy overlay
- **Points:** 3 · **Labels:** `ui` · **P1**
- **AC:**
  - [ ] Vertical markers on latency charts

### E4-S8 — Exception events on spans
- **Points:** 3 · **Labels:** `gem`, `server` · **P1**
- **AC:**
  - [ ] Exception type/message/stack truncated
  - [ ] Visible in waterfall detail panel

### E4-S9 — Seed intentional N+1 in dummy app
- **Points:** 2 · **Labels:** `infra` · **P1**
- **AC:**
  - [ ] Fixture endpoint demonstrates detection e2e

---

# E5 — Production hardening

**Goal:** Safe for staging/production continuous use.

### E5-S1 — Overhead benchmark harness
- **Points:** 5 · **Labels:** `perf`, `infra` · **P1**
- **AC:**
  - [ ] Script compares gem on/off
  - [ ] Results documented; CI can fail on regression threshold

### E5-S2 — Retention / TTL worker
- **Points:** 5 · **Labels:** `server` · **P1**
- **AC:**
  - [ ] Deletes traces older than config
  - [ ] Metrics retained longer
  - [ ] Disk usage bounded in soak test

### E5-S3 — UI authentication
- **Points:** 5 · **Labels:** `server`, `security` · **P1**
- **AC:**
  - [ ] Basic auth or single shared token for MVP
  - [ ] Documented env vars

### E5-S4 — Cardinality guards
- **Points:** 3 · **Labels:** `server`, `agent` · **P1**
- **AC:**
  - [ ] Reject/limit too-long resources
  - [ ] Cap attributes size

### E5-S5 — Adaptive sampling advice
- **Points:** 5 · **Labels:** `agent`, `gem` · **P2**
- **AC:**
  - [ ] Agent response can lower client sample rate under load

### E5-S6 — Soak test (24h) checklist
- **Points:** 3 · **Labels:** `perf` · **P1**
- **AC:**
  - [ ] Runbook executed once
  - [ ] Notes filed for leaks/growth

### E5-S7 — Structured logging & panic safety
- **Points:** 2 · **Labels:** `agent`, `server` · **P1**
- **AC:**
  - [ ] JSON or consistent logs
  - [ ] No panics on malformed payloads (return 400)

### E5-S8 — Security review checklist
- **Points:** 2 · **Labels:** `security`, `docs` · **P1**
- **AC:**
  - [ ] PII, auth, path traversal on static UI checked

---

# E6 — Packaging, release & documentation

**Goal:** Others can install without reading the monorepo.

### E6-S1 — Cross-compile / release binaries
- **Points:** 5 · **Labels:** `infra` · **P1**
- **AC:**
  - [ ] Linux amd64/arm64 + macOS builds
  - [ ] GitHub releases automated

### E6-S2 — Container image
- **Points:** 3 · **Labels:** `infra` · **P1**
- **AC:**
  - [ ] Dockerfile multi-stage
  - [ ] compose demo works cold

### E6-S3 — RubyGems publish pipeline
- **Points:** 3 · **Labels:** `gem`, `infra` · **P1**
- **AC:**
  - [ ] Version bump process documented
  - [ ] Trusted publishing or manual steps

### E6-S4 — User documentation site / pages
- **Points:** 5 · **Labels:** `docs` · **P1**
- **AC:**
  - [ ] Install guide
  - [ ] Configuration reference
  - [ ] Sampling & PII pages
  - [ ] Troubleshooting

### E6-S5 — Deploy platform recipes
- **Points:** 3 · **Labels:** `docs` · **P2**
- **AC:**
  - [ ] At least one of: Fly, Render, Docker VPS, K8s sidecar sketch

### E6-S6 — Public README polish
- **Points:** 2 · **Labels:** `docs` · **P1**
- **AC:**
  - [ ] Screenshots of UI
  - [ ] Quickstart under 20 lines

---

# E7 — Scale & OpenTelemetry interop

**Goal:** Growth path; not blocking MVP.

### E7-S1 — OTLP/HTTP ingest
- **Points:** 8 · **Labels:** `agent` · **P2**
- **AC:**
  - [ ] Accept OTLP traces
  - [ ] Map to internal model

### E7-S2 — ClickHouse storage backend
- **Points:** 13 · **Labels:** `server` · **P2**
- **AC:**
  - [ ] Feature-flagged backend
  - [ ] Migration guide from SQLite

### E7-S3 — Agent → remote server multi-host
- **Points:** 5 · **Labels:** `agent`, `server` · **P2**
- **AC:**
  - [ ] Documented topology
  - [ ] TLS options

### E7-S4 — Optional OTLP export
- **Points:** 5 · **Labels:** `agent` · **P3**
- **AC:**
  - [ ] Dual-write to external collector

### E7-S5 — Continuous profiling spike
- **Points:** 8 · **Labels:** `perf` · **P3**
- **AC:**
  - [ ] Spike doc: Vernier integration or external sampler
  - [ ] Go/no-go decision ADR

---

# E8 — DX, examples & dogfooding

**Goal:** Keep the path paved while building.

### E8-S1 — Makefile / justfile developer commands
- **Points:** 2 · **Labels:** `infra` · **P0**
- **AC:**
  - [ ] `just dev` or `make dev` runs server + dummy app instructions

### E8-S2 — CONTRIBUTING.md
- **Points:** 1 · **Labels:** `docs` · **P1**
- **AC:**
  - [ ] Build, test, style guide

### E8-S3 — Dogfood against internal app
- **Points:** 5 · **Labels:** `perf` · **P1**
- **AC:**
  - [ ] Install on one real app post-E3
  - [ ] File bugs as stories

### E8-S4 — Demo script / GIF
- **Points:** 2 · **Labels:** `docs` · **P2**
- **AC:**
  - [ ] Recorded happy path for README

---

## Suggested first sprint (sample)

**Sprint goal:** Scaffold + span context + agent ingest hello world.

| Story | Points |
|-------|--------|
| E0-S1 Cargo workspace | 3 |
| E0-S2 Gem skeleton | 3 |
| E0-S3 Dummy Rails | 5 |
| E0-S4 CI | 3 |
| E1-S1 Config DSL | 2 |
| E1-S2 Trace context | 5 |
| E1-S3 Rack root span | 3 |
| E1-S9 Stdout exporter | 2 |
| E2-S2 Protocol crate | 3 |
| E8-S1 Dev commands | 2 |
| **Total** | **~31** |

---

## Story template (copy into Jira)

```text
Title: [E#-S#] Short name
Epic: E#
Priority: P0|P1|P2|P3
Labels: ...
Points: ...

## Summary
<what / why>

## Acceptance criteria
- [ ] ...

## Technical notes
- ...

## Dependencies
- Blocked by: ...
- Blocks: ...

## Test plan
- ...
```

---

## Epic → component ownership (for staffing)

| Epic | Primary | Secondary |
|------|---------|-----------|
| E0 | Infra | All |
| E1 | Ruby | — |
| E2 | Rust | Protocol |
| E3 | Rust + UI | — |
| E4 | Ruby + UI + Server | — |
| E5 | All | Perf |
| E6 | Infra + Docs | — |
| E7 | Rust | — |
| E8 | Docs | All |

---

*Backlog version: 1.0 — 2026-07-10*  
*Story count: ~60 across 9 epics*
