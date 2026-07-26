# Railspan UI

Vue 3 + TypeScript + Vue Router SPA for the Railspan server.

## Develop

One command from the repo root:

```bash
just dev
# → API  http://127.0.0.1:7421   (no production SPA; / redirects to Vite)
# → UI   http://127.0.0.1:5173   ← open this (hot reload)
# Does not run `npm run build` or write static/assets.
```

## Production embed (optional)

```bash
just serve
# npm run build → crates/railspan-server/static, embed + serve on :7421
```

Or build assets only:

```bash
just ui-build
# → crates/railspan-server/static/
```

Routes: `/`, `/jobs`, `/n-plus-one`, `/deploys`, `/resources/:resource`, `/traces/:traceId`.
Query `?hours=24` is shared for the time range.
