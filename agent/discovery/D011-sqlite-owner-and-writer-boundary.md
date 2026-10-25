# D011 — Legacy schema initialisation cannot be shared with the native writer

Date: 2026-10-25. State: verified and resolved by the native ownership/writer boundary. Affected: T004/T005.

## Finding and evidence

`atlas.py:Store.__init__` executes `PRAGMA user_version=1` every time it opens the legacy database. Merely sharing its file with a version-2 Rust writer would let ordinary reference startup change the native schema marker. A sidecar lock used only by Rust would not stop Python's SQLite writes.

## Consequence

Use a separate native-owned file and refuse direct native writes to an unmarked legacy database. Migrate through a consistent SQLite backup into a separate target, retain a full version-1 recovery copy, validate before publication and never overwrite an existing destination. Native ownership/schema checks and a retained SQLite exclusive connection lock must prevent a second SQLite writer while the native store is open.

## Follow-up

T004 exercised refusal of the legacy file, failed migration and rollback. An actual separate Python SQLite writer was denied while Rust owned both synthetic and reference-copy native files. The reference's original hash stayed unchanged and its configured path was not changed. See [F02 acceptance](../evidence/F02-acceptance.md). [SQLite's locking-mode contract](https://www.sqlite.org/pragma.html#pragma_locking_mode) describes the retained connection lock used here.
