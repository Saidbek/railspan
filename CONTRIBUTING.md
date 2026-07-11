# Contributing

## Setup

- Rust stable (1.80+)
- Ruby 3.2+
- Optional: [just](https://github.com/casey/just)

```bash
cargo test --workspace
cd gem/railspan && bundle install && bundle exec rake test
```

## Workflow

1. Prefer vertical slices (instrumentation → agent → store/UI).
2. Every user-facing behavior should map to a story in `docs/BACKLOG.md`.
3. Architecture changes: add an ADR under `docs/adrs/`.
4. Keep the gem **fail-open** (never break the app request path).

## Style

- Rust: `cargo fmt`, `cargo clippy -- -D warnings`
- Ruby: clear names, minitest for unit tests
