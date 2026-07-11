# Product Definition

## Problem

Rails teams need production performance visibility:

- Which endpoints are slow (p95/p99)?
- Why is this request slow (SQL, views, external HTTP)?
- Where are N+1 queries?
- Are Sidekiq/ActiveJob workers healthy?

**Datadog / New Relic** solve this but are expensive, heavy, and general-purpose.  
**Skylight / AppSignal / Scout** are Rails-aware but SaaS-locked and closed.  
**OTel + Grafana/SigNoz** are powerful but generic — weak Rails product UX (N+1, AR, jobs).

## Solution

**Railspan**: self-hosted APM specialized for Rails.

- Drop-in Ruby gem (ActiveSupport::Notifications + Rack + jobs)
- Ultra-light Rust agent/server
- Opinionated UI: endpoints, waterfalls, N+1, jobs, deploy markers

## Personas

| Persona | Need | Success metric |
|---------|------|----------------|
| Solo / indie Rails founder | See slow pages without $200+/mo | Setup &lt; 15 min; useful in first hour |
| Agency / multi-app shop | One binary per env; cheap retention | Docker compose; multi-project API keys |
| Platform eng at mid-size SaaS | Low overhead; exportable data | &lt;2% CPU; OTLP export optional |
| Developer debugging prod | Trace waterfall + N+1 | Time-to-root-cause &lt; 5 min |

## Jobs to be done

1. “Show me the slowest endpoints this week.”
2. “Open a slow request and show me the waterfall.”
3. “Tell me if this is N+1 or one fat query.”
4. “Is Sidekiq lagging after the deploy?”
5. “Compare latency before/after this release.”

## Positioning

```text
                    Rails-specific UX
                           ▲
                           │
              Skylight     │     ★ Railspan
              AppSignal    │       (self-host + OSS)
              Scout        │
                           │
     ──────────────────────┼──────────────────────►
     SaaS-only             │              Self-host
                           │
              Datadog      │     SigNoz / Uptrace
              New Relic    │     Grafana stack
                           │
                           ▼
                    General observability
```

**Tagline options**

- “Rails APM without the bill.”
- “See every slow request. Own your data.”
- “Skylight-depth. Self-hosted.”

## Scope boundaries

### In scope (product north star)

- HTTP request tracing (Rack + Action Controller)
- ActiveRecord SQL spans (normalized)
- View / partial render spans
- Cache hit/miss spans
- Outbound HTTP client spans
- ActiveJob / Sidekiq job traces
- Endpoint RED metrics (Rate, Errors, Duration)
- N+1 detection
- Deploy / release markers
- Self-hosted single binary or docker compose
- API keys / multi-project (light multi-tenancy)
- Scrubbing of sensitive data

### Out of scope (v1 / explicit non-goals)

| Non-goal | Why | Revisit when |
|----------|-----|--------------|
| Full host/infra metrics | Not Rails APM | After product-market fit |
| Log management | Huge product; use Loki | Later integration |
| RUM / browser monitoring | Different SDK surface | v3+ |
| Security / ASM | Different threat model | Never unless spin-off |
| Non-Ruby languages | Dilutes focus | Only if OTLP ingest is free win |
| AI anomaly black box | Hard to trust; ship heuristics first | After solid metrics |
| Mobile APM | Out of domain | Never |

### Competitive differentiation (moat)

1. N+1 and duplicate-query UX as first-class, not custom PromQL  
2. ActiveRecord / view partial awareness  
3. Sidekiq + web in one Rails-shaped model  
4. Single-binary local dogfooding (`railspan serve`)  
5. Overhead budget enforced in CI against a sample app  

## Success metrics (project)

| Metric | Target |
|--------|--------|
| Time to first useful dashboard (new user) | ≤ 15 minutes |
| Instrumented app overhead (p99 latency) | &lt; 2% or &lt; 5ms absolute on simple endpoints |
| Agent RSS (single app, moderate traffic) | &lt; 100 MB |
| Query: “top endpoints last 24h” | &lt; 500ms for 7d local SQLite scale |
| N+1 precision on fixture app | Catch all seeded N+1s; &lt;5% false positives |

## Pricing model (later; not required for OSS launch)

| Tier | Model |
|------|--------|
| OSS self-host | Free forever |
| Hosted Cloud (optional) | Per-span or per-host retention tiers |
| Support | Optional commercial support |

Do **not** design multi-tenant cloud before OSS MVP works on a real Rails app.
