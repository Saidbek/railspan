# Railspan

**Lightweight, Rails-first APM** — request traces, SQL spans, and export to a Rust agent. Self-hosted. Open source.

> Not a Datadog clone. A focused performance product for Ruby on Rails teams.

**Status:** Phase 0 + Phase 1 in progress (gem instrumentation + agent ingest). Storage/UI (E3) next.

## Quick start

### 1. Run the agent

```bash
cargo run -p railspan-cli -- serve --addr 127.0.0.1:7421
# health: curl http://127.0.0.1:7421/healthz
```

### 2. Instrument a Rails app

```ruby
# Gemfile
gem "railspan", path: "path/to/railspan/gem/railspan"
```

```ruby
# config/initializers/railspan.rb
Railspan.configure do |c|
  c.service_name = "my-app"
  c.endpoint     = "http://127.0.0.1:7421"
  c.exporter     = :http # or :stdout for local debug
  c.enabled      = true
end
```

### 3. Dogfood with the dummy app

```bash
cd examples/dummy_rails
bundle install
bin/rails db:prepare db:seed

# terminal A
cargo run -p railspan-cli -- serve

# terminal B
RAILSPAN_EXPORTER=http bin/rails server -p 3000
curl localhost:3000/users
curl localhost:3000/users/with_posts   # intentional N+1
```

Stdout-only (no agent):

```bash
RAILSPAN_EXPORTER=stdout bin/rails server -p 3000
```

## Developer commands

```bash
just test          # cargo test + gem tests
just serve         # run agent
just test-gem
just test-rust
```

Or without `just`:

```bash
cargo test --workspace
cd gem/railspan && bundle exec rake test
```

## Repository layout

```text
railspan/
├── crates/                 # Rust workspace
│   ├── railspan-cli/       # `railspan serve`
│   ├── railspan-agent/     # HTTP ingest
│   ├── railspan-server/    # query/storage (stub → E3)
│   └── railspan-protocol/  # shared types
├── gem/railspan/           # Ruby instrumentation SDK
├── examples/dummy_rails/   # Rails 8 dogfood app
├── docker/
└── docs/                   # product plan, architecture, backlog
```

## What works today

| Feature | Status |
|---------|--------|
| Rack root span | ✅ |
| Action Controller spans | ✅ |
| ActiveRecord SQL spans + normalize | ✅ |
| Action View spans | ✅ (when views render) |
| PII scrubber | ✅ |
| Stdout exporter | ✅ |
| HTTP batch export to agent | ✅ |
| Agent `POST /v1/traces` | ✅ |
| Agent health metrics | ✅ |
| Persistence / UI | ⏳ E3 |
| N+1 detector / Sidekiq | ⏳ E4 |

## Docs

| Doc | Path |
|-----|------|
| Master plan | [`docs/PLAN.md`](docs/PLAN.md) |
| Architecture | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| Backlog | [`docs/BACKLOG.md`](docs/BACKLOG.md) |
| Protocol | [`docs/PROTOCOL.md`](docs/PROTOCOL.md) |

## License

MIT — see [LICENSE](LICENSE).
