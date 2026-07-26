# Security review checklist (Phase 5)

Complete before exposing Railspan beyond a trusted network.

## Auth

| Item | Status |
|------|--------|
| Ingest (`/v1/*`) requires `RAILSPAN_API_KEY` Bearer in non-local deploys | ☐ |
| Query API (`/api/*`) requires UI token (`RAILSPAN_UI_TOKEN` or same API key) | ☐ |
| `/healthz` may stay public (no secrets in body) | ☐ |
| Tokens not committed to git / baked into public images | ☐ |

## PII & data

| Item | Status |
|------|--------|
| Gem scrubber defaults cover password/token/secret keys | ☐ |
| SQL literals normalized to `?` before export | ☐ |
| Attribute values truncated server-side | ☐ |
| No raw secrets in span attributes during dogfood review | ☐ |

## Surface area

| Item | Status |
|------|--------|
| UI is static HTML only (no server-side template injection) | ☐ |
| Path traversal: static UI is `include_str!`, not user paths | ☐ |
| CORS is permissive for MVP — tighten if browser cross-origin is not needed | ☐ |
| Max body / max spans per batch enforced (413) | ☐ |
| Malformed JSON returns 400 without panic | ☐ |

## Ops

| Item | Status |
|------|--------|
| Retention enabled (`--retention-days`) so disks cannot grow forever | ☐ |
| Logs use `--log-format json` in prod aggregators; no API keys logged | ☐ |
| TLS terminated at reverse proxy if exposed beyond localhost | ☐ |

## Residual risks (accepted for MVP)

- API keys compared as plaintext shared secrets (not hashed at rest yet)
- Single-tenant SQLite file permissions depend on host OS
- UI token stored in browser `sessionStorage` when using Auth button
