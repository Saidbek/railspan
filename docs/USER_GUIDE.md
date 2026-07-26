# Railspan User Guide

## Install (monorepo)

```bash
git clone https://github.com/Saidbek/railspan.git
cd railspan
cargo build -p railspan-cli --release
./target/release/railspan serve --addr 127.0.0.1:7421 --data-dir ./data
```

Open http://127.0.0.1:7421

## Instrument a Rails app

```ruby
# Gemfile
gem "railspan", path: "/path/to/railspan/gem/railspan"
```

```ruby
# config/initializers/railspan.rb
Railspan.configure do |c|
  c.service_name = "my-app"
  c.environment  = Rails.env
  c.endpoint     = ENV.fetch("RAILSPAN_ENDPOINT", "http://127.0.0.1:7421")
  c.api_key      = ENV["RAILSPAN_API_KEY"]
  c.exporter     = :http # :stdout for debug
  c.enabled      = true
end
```

## What is instrumented

- Rack / Action Controller
- ActiveRecord SQL (normalized fingerprints)
- Action View templates/partials
- ActiveSupport::Cache
- Net::HTTP clients
- ActiveJob perform/enqueue
- Sidekiq server/client middleware (if Sidekiq is loaded)

## N+1 detection

On ingest, identical SQL fingerprints (≥ 5 by default, `--n1-threshold`) in one trace are flagged. UI shows badges on endpoints and a dedicated N+1 tab.

## Deploys

```bash
curl -X POST http://127.0.0.1:7421/v1/deploys \
  -H 'content-type: application/json' \
  -d '{"git_sha":"abc123","version":"v1.2.3"}'
```

Or from Ruby:

```ruby
Railspan.record_deploy!(git_sha: ENV["GIT_SHA"], version: ENV["APP_VERSION"])
```

## Server flags / ENV

| Flag | ENV | Default |
|------|-----|---------|
| `--addr` | `RAILSPAN_INGEST_ADDR` | `127.0.0.1:7421` |
| `--data-dir` | `RAILSPAN_DATA_DIR` | `./data` |
| `--api-key` | `RAILSPAN_API_KEY` | none |
| `--ui-token` | `RAILSPAN_UI_TOKEN` | same as API key |
| `--sample-rate` | `RAILSPAN_SAMPLE_RATE` | `1.0` |
| `--slow-ms` | `RAILSPAN_SLOW_MS` | `500` |
| `--retention-days` | `RAILSPAN_RETENTION_DAYS` | `7` |
| `--n1-threshold` | `RAILSPAN_N1_THRESHOLD` | `5` |
| `--log-format` | `RAILSPAN_LOG_FORMAT` | `text` (`json` supported) |
| `--source-root` | `RAILSPAN_SOURCE_ROOT` | none (code highlight off) |

### Auth

- **Ingest** `POST /v1/*`: Bearer `RAILSPAN_API_KEY` when set.
- **Query API** `GET /api/*`: Bearer `RAILSPAN_UI_TOKEN`, or the API key if UI token is unset.
- **UI**: open HTML; click **Auth** to store a token in `sessionStorage` (sent on API calls).
- **Health** `/healthz`: always open.

### Sampling

- **Server:** always keeps error and slow roots; other traces kept with probability `sample_rate`.
- **Adaptive advice:** ingest responses include `advice.sample_rate` under load; the gem may lower client head-sampling.
- **Gem head sampling:** `sample_rate` can skip instrumenting some requests/jobs entirely.

### Source locations & code highlight

The gem attributes SQL / cache / HTTP client / custom spans to **application** `file:line` (`code.filepath`, `code.lineno`, `code.function`). N+1 events inherit the first location for the repeated query.

**Gem**

| Setting / ENV | Default |
|---------------|---------|
| `capture_source_location` / `RAILSPAN_CAPTURE_SOURCE_LOCATION` | `true` |
| `source_location_kinds` / `RAILSPAN_SOURCE_LOCATION_KINDS` | `sql,cache,http.client,custom` |
| `application_root` / `RAILSPAN_APPLICATION_ROOT` | `Rails.root` when present |

**Server:** set `--source-root` to the Rails app directory so the UI can open snippets via `GET /api/v1/source`. Without it, path:line still shows; highlight is unavailable.

```bash
railspan serve --source-root ./examples/dummy_rails
# UI: open a trace → click a SQL span → code panel
```

Design notes: [SOURCE_LOCATIONS.md](./SOURCE_LOCATIONS.md).

### Hardening

- Retention worker deletes traces older than `--retention-days` (hourly).
- Batch limits: max 5000 spans, body ≤ 16 MiB → HTTP 413.
- Attribute/event cardinality caps on ingest.
- Overhead microbench: `./scripts/bench_overhead.sh` (see [OVERHEAD.md](./OVERHEAD.md)).
- Soak + security: [runbooks/SOAK.md](./runbooks/SOAK.md), [SECURITY_CHECKLIST.md](./SECURITY_CHECKLIST.md).

## API

| Method | Path |
|--------|------|
| POST | `/v1/traces` |
| POST | `/v1/deploys` |
| GET | `/healthz` |
| GET | `/api/v1/endpoints?hours=24` |
| GET | `/api/v1/traces?resource=&hours=24` |
| GET | `/api/v1/traces/:id` |
| GET | `/api/v1/n-plus-one` |
| GET | `/api/v1/source?path=&line=&context=` |
| GET | `/api/v1/deploys` |
| GET | `/api/v1/stats` |
| GET | `/` UI (Vue SPA; also `/jobs`, `/n-plus-one`, `/deploys`, `/resources/…`, `/traces/…`) |

### UI (Vue)

Source lives in `ui/` (Vue 3 + TypeScript + Vue Router).

```bash
just dev      # local: API-only :7421 + Vite :5173 (no npm build / no embedded SPA)
just serve    # production: npm build → embed UI on :7421 only
just ui-build # only rebuild crates/railspan-server/static
```

With `just dev`, `:7421` redirects browser UI paths to Vite (`--dev-ui-url`); open **http://127.0.0.1:5173**.

## Docker

```bash
docker build -f docker/Dockerfile -t railspan .
docker run --rm -p 7421:7421 -v railspan-data:/data \
  -e RAILSPAN_INGEST_ADDR=0.0.0.0:7421 \
  -e RAILSPAN_DATA_DIR=/data \
  railspan serve
```
