# Railspan

**Lightweight, Rails-first APM** — traces, SQL, N+1 detection, jobs, and a built-in UI.  
Self-hosted. Open source. Rust agent/server + Ruby gem.

> Not a Datadog clone. Skylight-depth for teams who want to own their data.

**Status:** MVP complete (instrumentation → serve → SQLite → UI). Production hardening ongoing.

## Quick start

```bash
# 1) Server + UI
cargo run -p railspan-cli -- serve --addr 127.0.0.1:7421 --data-dir ./data
# open http://127.0.0.1:7421

# 2) Dummy Rails app
cd examples/dummy_rails
bundle install && bin/rails db:prepare db:seed
RAILSPAN_EXPORTER=http RAILSPAN_ENDPOINT=http://127.0.0.1:7421 bin/rails s -p 3000
curl localhost:3000/users
curl localhost:3000/users/with_posts   # triggers N+1
```

## Features

| Feature | Status |
|---------|--------|
| Request / controller / SQL / view spans | ✅ |
| Cache + Net::HTTP | ✅ |
| ActiveJob + Sidekiq | ✅ |
| SQL normalize + PII scrub | ✅ |
| HTTP batch export + stdout | ✅ |
| SQLite persistence | ✅ |
| Endpoints p50/p95/p99 | ✅ |
| Trace waterfall UI | ✅ |
| N+1 detection + UI | ✅ |
| Jobs dashboard | ✅ |
| Deploy markers | ✅ |
| Sampling + retention | ✅ |
| OTLP / ClickHouse | ⏳ future |

## Docs

- [User guide](docs/USER_GUIDE.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Plan](docs/PLAN.md)
- [Backlog](docs/BACKLOG.md)
- [Protocol](docs/PROTOCOL.md)
- [Contributing](CONTRIBUTING.md)

## Layout

```text
crates/   railspan-cli, server, agent, protocol
gem/      Ruby SDK
examples/ dummy_rails
docs/     design + guides
```

## License

MIT — see [LICENSE](LICENSE).
