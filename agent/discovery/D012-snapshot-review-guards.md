# D012 — SQLite schema and size guards must inspect the right representation

Date: 2026-10-25. State: resolved by regressions and independent follow-up review. Affected: T004/T005.

## Finding and evidence

A focused independent F02 review found two bounded migration defects. SQL `LIKE 'sqlite_%'` treats the underscore as a wildcard, allowing a user table such as `sqliteExtra` to bypass the intended unknown-table rejection. A main-file size check also misses committed pages held in a WAL file; the consistent backup can exceed the documented 64 MiB limit.

## Consequence

Use a literal internal-name prefix (`GLOB 'sqlite_*'`). Bound backup progress by source page count/page size and check the completed snapshot file before recovery publication or row loading. Keep source/recovery/no-clobber guarantees intact; do not silently omit unknown owner data to complete migration.

## Follow-up

Regressions passed for a real `sqliteExtra` table and an open WAL source using a small injected limit, avoiding a large throwaway fixture. The affected synthetic/reference-copy parity, rollback and writer-lock journey passed again with the original hash unchanged. The reviewer confirmed both findings resolved with no remaining material issue in these fixes; see [F02 acceptance](../evidence/F02-acceptance.md). No real reference was changed.
