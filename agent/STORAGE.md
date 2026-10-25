# Native SQLite ownership, migration and recovery

The Rust library is in `crates/atlas-core`. The native-owned database has application ID `0x41544c53` and schema version 2. The legacy database has application ID 0/version 1. Native `Store::open` refuses the legacy marker and retains an exclusive SQLite connection lock until closed, preventing concurrent SQLite writers, including the Python reference. Do not configure both apps to share a file.

Migration opens its source read-only, creates a consistent SQLite backup and retains a full legacy recovery snapshot. It validates and rebuilds canonical tables in a private staging file, preserving IDs, owner edits, coordinates, relationships, supported activity history and classified baseline records. Only then is a separate destination published without overwriting existing files. Source/recovery files remain unchanged. Validation failures leave no published native target and retain recovery material. Unknown tables/views/triggers require investigation rather than execution or silent omission.

The source is limited to 64 MiB, with bounded backup attempts and limits of 10,000 servers, 100,000 relationships and 10,000 history entries. The current four-host reference is well below those limits. Unsupported data is rejected recoverably, never truncated to make a check pass. Native schema/data paths are not automatically selected by the UI or cut over from the reference yet.

Developer commands (explicit paths, never the only live file):

```powershell
mise run exec -- cargo build -p atlas-core --bin atlas-data --locked
mise run exec -- target/debug/atlas-data.exe create .tools/new-native.sqlite3
mise run exec -- target/debug/atlas-data.exe migrate .tools/legacy-copy.sqlite3 .tools/migrated-native.sqlite3 .tools/recovery.sqlite3
mise run exec -- target/debug/atlas-data.exe restore .tools/recovery.sqlite3 .tools/restored-legacy.sqlite3
mise run test:integration
mise run check:android-core
```

`restore` creates a new version-1 file; it never overwrites the original or switches coordinator configuration. Returning to the Python reference uses its original untouched database. Release cutover/rollback instructions and acceptance remain T024–T026.

`scripts/verify_data.py` uses the actual Python reference and Rust CLI on synthetic fixtures by default. Its optional `--legacy-source data/atlas.sqlite3` path opens that original read-only only to make a private backup; all comparison/reference writes, migration and restore occur on copies. Successful and failing proof copies stay under ignored `.tools/` for investigation. It prints counts/results only, not private inventory values. No recursive cleanup of owner data occurs.

Imports load only the configured four saved-report filenames, with bounded reads, validated retained fields, stable source identity and one transaction per batch. New/equal/older times compare actual instants, not timestamp text order. Conflicting source-key/hostname identities reject the whole batch instead of transferring ownership between records. See [INVENTORY.md](INVENTORY.md) for retained/excluded fields. The CLI is a local developer utility, not an unauthenticated network API.
