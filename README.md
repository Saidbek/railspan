# Railspan

**Lightweight, Rails-first APM** — traces, SQL, N+1 detection, jobs, and a built-in UI.  
Self-hosted. Open source. Rust agent/server + Ruby gem.

**Status:** MVP + Phase 5 hardening complete. Packaging (Phase 6) next.

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

## Demo

<img width="1512" height="340" alt="Screenshot 2026-07-25 at 9 21 47 PM" src="https://github.com/user-attachments/assets/7c97e7fd-fe8e-4c8a-9c48-5a756d2a55a2" />
<img width="1512" height="864" alt="Screenshot 2026-07-25 at 9 22 05 PM" src="https://github.com/user-attachments/assets/5b30e3a1-dbd2-4e4b-9604-c317baa92fc0" />

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
| UI/API auth + adaptive sampling | ✅ |
| Overhead bench + soak/security docs | ✅ |
| Source locations + code highlight | ✅ |
| OTLP / ClickHouse | ⏳ future |

## Docs

- [User guide](docs/USER_GUIDE.md)
- [Source locations](docs/SOURCE_LOCATIONS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Plan](docs/PLAN.md)
- [Backlog](docs/BACKLOG.md)
- [Protocol](docs/PROTOCOL.md)
- [Contributing](CONTRIBUTING.md)

## Layout

```text
crates/   railspan-cli, server, agent, protocol
ui/       Vue 3 + TypeScript SPA (build → crates/railspan-server/static)
gem/      Ruby SDK
examples/ dummy_rails
docs/     design + guides
```

### UI development

```bash
just dev          # API-only :7421 + Vite HMR :5173 — open http://127.0.0.1:5173
                  # (does not run npm build / does not serve static/assets on :7421)
just serve        # production: npm build → embed Vue on :7421 only
```

## License

MIT — see [LICENSE](LICENSE).
