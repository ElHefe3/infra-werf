# D042 — Existing native Atlas store and recovery snapshot

Date: 2026-10-27<br>
State: verified<br>
Affected: T026

## Finding

A read-only preflight found `%APPDATA%\dev.atlas.workspace\atlas.sqlite3` and one `legacy-recovery-*.sqlite3` file, both timestamped 2026-10-25. The reference `data/atlas.sqlite3` still matches its saved SHA-256 baseline (`2CD22BD9AFBE8BF95481C5FA2D33A9A4773BEF6743D278875FB2CB68316260CE`). Subsequent `immutable=1` SQLite checks found all three databases structurally healthy. The recovery copy matches the reference's logical rows; the native store matches the reference's allowlisted inventory projection, with the same four server records, 504 services, 36 containers, edges and events.

## Evidence

The T025 package journey used `ATLAS_NATIVE_DB` pointing to a generated `.tools` fixture. T026 used read-only, immutable SQLite connections for integrity checks and a comparison that emitted only aggregate counts and equality flags; no row values or identifiers were emitted and no database was changed. The native report intentionally omits legacy `warnings`, service `boot`/`restart` and container `restart` fields. `BaselineInventory` is an allowlisted contract, and the synthetic migration proof verifies excluded free-form fields are not persisted. The Windows Computer Use bridges could not initialize in this continuation, so the actual app-data database was not inspected through Atlas's UI.

## Consequence

Do not run a first migration over the existing native database or delete the recovery file. Data integrity and allowlisted parity are technically verified; T026 still needs owner review that the installed app displays the intended map and owner acceptance before treating the installed release as primary. Server 1 remains offline and must not be probed.

## Follow-up

Owner review of the installed Windows app and the approved host list remains pending; see [T026 owner checklist](../evidence/T026-owner-checklist.md).
