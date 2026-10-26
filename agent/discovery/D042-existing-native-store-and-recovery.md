# D042 — Existing native Atlas store and recovery snapshot

Date: 2026-10-27<br>
State: verified<br>
Affected: T026

## Finding

Before any T026 launch or migration, a read-only preflight found `%APPDATA%\dev.atlas.workspace\atlas.sqlite3` and one `legacy-recovery-*.sqlite3` file. Both were timestamped 2026-10-25. Their contents were not opened, hashed, copied, or changed. The project reference `data/atlas.sqlite3` still matches its saved SHA-256 baseline (`2CD22BD9AFBE8BF95481C5FA2D33A9A4773BEF6743D278875FB2CB68316260CE`).

## Evidence

The T025 package journey used `ATLAS_NATIVE_DB` pointing to a generated `.tools` fixture. The preflight used only `Test-Path`, file names, sizes and modification times for the app-data directory, plus a SHA-256 read of the project reference database. The Windows Computer Use bridges could not initialize in this continuation, so the actual app-data database was not inspected through Atlas.

## Consequence

Do not run a first migration over the existing native database or delete the recovery file. T026 must verify in the installed app that the displayed map is the intended copy, retain the reference and recovery data, and record owner acceptance before treating the installed release as primary. Server 1 remains offline and must not be probed.

## Follow-up

Owner review of the installed Windows app and the approved host list remains pending; see [T026 owner checklist](../evidence/T026-owner-checklist.md).
