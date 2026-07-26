# Soak test runbook (24h)

Use before calling a release “production-ready” for continuous staging use.

## Setup

```bash
cargo build -p railspan-cli --release
./target/release/railspan serve \
  --addr 0.0.0.0:7421 \
  --data-dir ./data/soak \
  --api-key "$RAILSPAN_API_KEY" \
  --sample-rate 0.2 \
  --retention-days 2 \
  --log-format json
```

Instrument `examples/dummy_rails` (or a staging app) with HTTP export to the soak server.

Generate continuous light traffic:

```bash
# simple loop
while true; do
  curl -s -o /dev/null http://127.0.0.1:3000/users
  curl -s -o /dev/null http://127.0.0.1:3000/users/with_posts
  sleep 1
done

# or production-like mixed load (preferred)
just load-test 60 12
# flood / adaptive-sampling probe
just load-flood 30 40
# see scripts/load_test.rb --help
```

## Checklist (fill during/after 24h)

| Check | Pass? | Notes |
|-------|-------|-------|
| Process still up after 24h | ☐ | |
| RSS stable (no unbounded growth) | ☐ | sample `ps` / metrics hourly |
| Disk under `data-dir` bounded | ☐ | retention deletes old traces |
| `/healthz` ok; batches_received increasing | ☐ | |
| No panic loops in logs | ☐ | malformed payloads → 400 |
| UI `/api/v1/endpoints` responds | ☐ | with Bearer if UI token set |
| Drop counters sane under load | ☐ | sample / cardinality drops OK |
| Adaptive `advised_sample_rate` moves under flood | ☐ | optional flood test |

## Flood probe (optional, 10–15 min)

Send large synthetic batches or high RPS. Confirm:

- No OOM
- 413 on oversized batches
- Advice `sample_rate` decreases under load
- Gem adopts lower rate when `RAILSPAN_DEBUG=1` logs show adaptation

## Sign-off

- Date:
- Operator:
- Build SHA:
- Result: pass / fail
- Follow-ups:
