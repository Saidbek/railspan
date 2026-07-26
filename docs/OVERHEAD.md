# Overhead budget

**Goal (G7):** instrumented-app overhead &lt; ~**2%** on typical request latency when sample rate is sensible.

## Microbench (CI-friendly)

```bash
./scripts/bench_overhead.sh
# or
ruby scripts/bench_overhead.rb 50000
```

Measures `Railspan::Tracer.in_span` create/finish with exporter `:null` vs gem disabled.

| Signal | Expectation |
|--------|-------------|
| Enabled path | Typically single-digit to low tens of µs/span on modern laptops |
| Soft CI fail | &gt; 500 µs/span (pathological regression only) |

This is **not** full request overhead; it guards the span hot path.

## Request-level (manual / soak)

1. Start `railspan serve`.
2. Run `examples/dummy_rails` with `RAILSPAN_ENABLED=0` and load a stable endpoint (`/users`) with `ab` or `wrk`.
3. Repeat with `RAILSPAN_ENABLED=1` and `RAILSPAN_EXPORTER=http`.
4. Compare p50/p95.

**Budget:** p95 regression ideally &lt; ~2% at `sample_rate=0.1`–`1.0` depending on SQL span volume.

| Lever | Effect |
|-------|--------|
| `sample_rate` / adaptive advice | Fewer roots instrumented / stored |
| `ignore_paths` | Skip health/assets |
| Server sampling + retention | Bound backend cost |

## Agent / server

- Bounded batch size (`MAX_SPANS_PER_BATCH = 5000`)
- Attribute/event cardinality caps
- Hourly retention worker (`--retention-days`, default 7)

See also [runbooks/SOAK.md](./runbooks/SOAK.md).
