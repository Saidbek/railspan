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
| `--sample-rate` | `RAILSPAN_SAMPLE_RATE` | `1.0` |
| `--slow-ms` | `RAILSPAN_SLOW_MS` | `500` |
| `--retention-days` | `RAILSPAN_RETENTION_DAYS` | `7` |
| `--n1-threshold` | `RAILSPAN_N1_THRESHOLD` | `5` |

Sampling always keeps error and slow roots; other traces kept with probability `sample_rate`.

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
| GET | `/api/v1/deploys` |
| GET | `/api/v1/stats` |
| GET | `/` UI |

## Docker

```bash
docker build -f docker/Dockerfile -t railspan .
docker run --rm -p 7421:7421 -v railspan-data:/data \
  -e RAILSPAN_INGEST_ADDR=0.0.0.0:7421 \
  -e RAILSPAN_DATA_DIR=/data \
  railspan serve
```
