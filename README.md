# Railspan

**Lightweight, Rails-first APM** — request traces, endpoint metrics, N+1 detection, and background job visibility. Self-hosted. Open source. Rust agent + Ruby gem.

> Not a Datadog clone. A focused performance product for Ruby on Rails teams who want Skylight-depth insight without SaaS bills or a heavy polyglot agent.

## Status

**Planning** — design and backlog first; implementation starts after epics/stories are filed.

| Artifact | Path |
|----------|------|
| Master plan | [`docs/PLAN.md`](docs/PLAN.md) |
| Architecture | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| Product & scope | [`docs/PRODUCT.md`](docs/PRODUCT.md) |
| Roadmap | [`docs/ROADMAP.md`](docs/ROADMAP.md) |
| Epics & stories (Jira-ready) | [`docs/BACKLOG.md`](docs/BACKLOG.md) |
| Data model | [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md) |
| Protocol | [`docs/PROTOCOL.md`](docs/PROTOCOL.md) |

## One-liner (target UX)

```bash
# Start local agent + UI + storage
railspan serve

# Gemfile
gem "railspan"
```

Open `http://localhost:4318` → endpoints, waterfalls, N+1 flags, Sidekiq jobs.

## High-level components

| Component | Language | Role |
|-----------|----------|------|
| `railspan` gem | Ruby | Rails / Rack / ActiveJob instrumentation |
| `railspan-agent` | Rust | Receive, sample, batch, aggregate |
| `railspan-server` | Rust | Persist, query API, serve UI assets |
| UI | TypeScript (or Hotwire later) | Dashboards & trace explorer |

## Repo layout (planned monorepo)

```text
railspan/
├── README.md
├── docs/                 # plans, ADRs, backlog
├── crates/               # Rust workspace
│   ├── railspan-agent/
│   ├── railspan-server/
│   ├── railspan-protocol/
│   └── railspan-cli/
├── gem/                  # Ruby gem
│   └── railspan/
├── ui/                   # Web UI
├── docker/               # compose for local/prod
└── examples/             # sample Rails apps
```

## Principles

1. **Rails-native depth** over polyglot breadth  
2. **&lt;2% overhead** budget on instrumented apps (measure, enforce)  
3. **Smart sampling** — always keep errors & slow; sample the rest  
4. **Self-host first**; optional hosted SaaS only after OSS product works  
5. **OTLP-friendly** wire format so we don’t paint into a corner  
6. **PII scrubbing on by default** (SQL params, request params)

## License (proposed)

MIT or Apache-2.0 — decide before first public release.

## Next steps

1. Review [`docs/PLAN.md`](docs/PLAN.md)  
2. File epics/stories from [`docs/BACKLOG.md`](docs/BACKLOG.md)  
3. Scaffold monorepo and implement Phase 0 / Phase 1  
