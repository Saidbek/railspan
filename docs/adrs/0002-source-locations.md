# ADR 0002: Source locations via span attributes

- **Status:** accepted
- **Date:** 2026-07-25

## Context

Users need line-level attribution for timeline events (especially SQL and N+1), comparable to other APMs, without a protocol rewrite.

## Decision

1. Capture app call sites in the **Ruby gem** using `caller_locations`, skip framework/gem frames.  
2. Ship as OTel-aligned span attributes: `code.filepath`, `code.lineno`, `code.function`.  
3. Default kinds: `sql`, `cache`, `http.client`, `custom`.  
4. Denormalize first location onto N+1 events.  
5. Serve optional snippets from a configured `--source-root` (no repo upload).  

Full write-up: [SOURCE_LOCATIONS.md](../SOURCE_LOCATIONS.md).

## Consequences

- No protocol version bump.  
- UI works without source root (shows path:line only).  
- Operators must point `source_root` at the matching app tree for highlight.  
