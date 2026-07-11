# ADR 0001: Initial project decisions

- **Status:** accepted
- **Date:** 2026-07-10

## Context

Railspan is starting implementation. Several product defaults need locking for MVP.

## Decision

1. **Name:** Railspan  
2. **License:** MIT  
3. **Min Ruby:** 3.2+  
4. **Min Rails (instrumentation target):** 7.1+ (MRI only for v1)  
5. **Default ingest port:** `7421`  
6. **UI framework:** deferred until E3 (API-first)  
7. **Storage MVP:** SQLite in E3; agent currently accepts and logs only  

## Consequences

Docs and scaffolds follow these defaults. Changing license or min versions requires a new ADR.
