# Source locations & line-level attribution

**Status:** accepted for implementation  
**Date:** 2026-07-25  
**Related:** [PROTOCOL.md](./PROTOCOL.md), [DATA_MODEL.md](./DATA_MODEL.md), ADR style decisions below

## Problem

Other APMs answer not only *“this endpoint is slow”* but *where in application code* a timeline event came from (file + line). Railspan already surfaces **what** happened (SQL fingerprint, N+1 repeats, span kinds) but not **where** it originated in the Rails app.

## Goals

1. Attribute high-value spans to **application** file/line/function when possible.
2. Propagate locations into **N+1 events** (most common / first SQL site).
3. Show locations in the **waterfall UI**, with optional **code highlight** (snippet ± context lines).
4. Keep hot-path cost low; never break requests if location capture fails.

## Non-goals (v1)

- Full stack traces on every span (too heavy, noisy).
- Uploading entire repositories to the server.
- Cross-host source maps for containerized deploys without a configured source root.
- Browser / JS source maps.

## Decision summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Transport | Span **attributes** using OTel-aligned keys | No protocol version bump; query API already returns attributes |
| Keys | `code.filepath`, `code.lineno`, `code.function` (+ optional `code.namespace`) | Matches OpenTelemetry code attributes; UI/API share one model |
| Capture site | Gem, at span start for selected kinds | Server cannot invent Ruby call sites |
| Kinds (default) | `sql`, `cache`, `http.client`, `custom` | Highest “why is this slow?” value; skip Rack/controller/view noise |
| Stack walk | `caller_locations`, skip gem/framework frames | Synchronous AS::Notifications keep app frames on the stack |
| App frame rule | Prefer path under `Rails.root` excluding `vendor/`; else reject `/gems/`, `vendor/bundle`, `lib/railspan/` | Avoid pointing at ActiveRecord internals |
| Relativize | Store path relative to `Rails.root` when possible | Stable UI labels across machines |
| Snippets at export | **No** by default | Disk I/O + secret risk on every SQL |
| Code highlight | Server `GET /api/v1/source` reads from `--source-root` / `RAILSPAN_SOURCE_ROOT` | Self-hosted: operator points at the app checkout; path traversal hardened |
| N+1 | Persist first non-empty location among SQL spans sharing the fingerprint | Explains *which line* looped associations |
| Config | `capture_source_location` (default **true**), overridable kinds / disable | Prod can turn off if needed; ENV `RAILSPAN_CAPTURE_SOURCE_LOCATION` |
| Fail-open | Capture errors → omit attributes | Never raise into the app |

## Attribute contract

On a span (example SQL):

```json
{
  "kind": "sql",
  "resource": "SELECT \"posts\".* FROM \"posts\" WHERE \"posts\".\"user_id\" = ?",
  "attributes": {
    "db.statement": "SELECT \"posts\".* FROM \"posts\" WHERE \"posts\".\"user_id\" = ?",
    "code.filepath": "app/controllers/users_controller.rb",
    "code.lineno": 18,
    "code.function": "with_posts"
  }
}
```

N+1 event (API) gains optional fields mirrored from that location:

```json
{
  "sql_fingerprint": "SELECT …",
  "repeat_count": 20,
  "code_filepath": "app/controllers/users_controller.rb",
  "code_lineno": 18,
  "code_function": "with_posts"
}
```

## Source snippet API

```http
GET /api/v1/source?path=app/controllers/users_controller.rb&line=18&context=5
Authorization: Bearer <ui token>
```

Rules:

- Requires `source_root` configured on the server.
- Resolve `source_root.join(path)` and require the canonical path still starts with `source_root`.
- Allow only regular files; default allowlist extension `.rb` (and `.rake`, `.arb` optional).
- Cap file size (e.g. 512 KiB) and context lines (max 20).
- Response:

```json
{
  "path": "app/controllers/users_controller.rb",
  "line": 18,
  "start_line": 13,
  "language": "ruby",
  "lines": ["  def with_posts", "    users = User.limit(20)", "    ..."]
}
```

If root unset or file missing → `404` with a clear message (UI still shows path:line as text).

## UI

1. Waterfall rows are **clickable**.
2. Detail panel shows kind, duration, key attributes, and **`path:line` in `function`**.
3. When source API succeeds, render a monospace block with the target line highlighted.
4. N+1 list/cards show the same path:line when present.

## Performance notes

- `caller_locations(n, 40)` only for opted-in kinds.
- No file reads on the request path in the gem.
- Location strings truncated (filepath ≤ 512 chars, function ≤ 128).

## Security notes

- Source API is behind UI auth when configured.
- Path traversal blocked; no symlink escape outside root.
- Do not log full file contents.
- Snippets may include secrets if developers hardcode them — same risk as opening the file in an editor; operators should only set `source_root` for trusted environments.

## Alternatives considered

| Alternative | Why not (for now) |
|-------------|-------------------|
| Dedicated span fields in protocol v2 | Attributes already ship; avoids dual models |
| Always attach full backtrace | Cardinality + size + scrubbing cost |
| Gem embeds code snippet on export | I/O and PII on every SQL; harder to toggle per environment |
| IDE deep links only | Less useful for shared self-hosted UI |

## Rollout

1. Gem capture + attributes  
2. N+1 columns + API  
3. UI detail + source highlight  
4. Docs / USER_GUIDE / CHANGELOG  

## Open follow-ups

- Deploy-aware multi-root (map `service` / git SHA → source tree).  
- Aggregate “hottest lines” view (group spans by `code.filepath`+`lineno`).  
- Sidekiq/ActiveJob job class method lines (optional).  
