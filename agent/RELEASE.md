# Windows desktop install, update and recovery

## Build and artifact

From the repository root, run `mise run build:desktop`. This uses the committed `mise.toml`, pnpm lockfile and Cargo lockfile to produce an optimized Windows x64 NSIS setup executable:

```text
target/release/bundle/nsis/Atlas_<version>_x64-setup.exe
```

`mise run smoke:desktop` produces a debug executable only; do not distribute it as a release installer. Tauri's Windows installer uses the current user by default, installs below `%LOCALAPPDATA%\Atlas`, and records uninstall information under the current user's registry hive. The NSIS package uses WebView2's download bootstrapper when a suitable runtime is missing; machines without WebView2 need internet access during installation. The release artifact does not embed the offline runtime.

The current T025 package is unsigned. Its SHA-256 is recorded in [T025 acceptance](evidence/T025-acceptance.md); this detects accidental file changes but does not prove publisher identity. Obtain and configure an approved code-signing certificate before wider distribution. Do not bypass Windows publisher or SmartScreen warnings.

## Saved data and first launch

The app identifier is `dev.atlas.workspace`. On Windows, the default Tauri app-data directory is `%APPDATA%\dev.atlas.workspace`; the native store is `atlas.sqlite3`, with any retained legacy recovery copy beside it. The desktop integration harness overrides `ATLAS_NATIVE_DB` to point only at its generated `.tools` database; do not set that variable for ordinary use.

The installer does not automatically import the Python reference database. The reference remains `data/atlas.sqlite3`. Existing native data takes precedence. T026 read-only checks confirmed that the existing `%APPDATA%\dev.atlas.workspace\atlas.sqlite3` and `legacy-recovery-*.sqlite3` are structurally healthy and preserve the allowlisted inventory/recovery data. Owner review that the installed app displays the intended map is still pending; do not initialize over the store or rerun migration. When no native store exists, any first migration must be explicitly reviewed; Atlas reads the source, writes a separate native database, and retains a recovery copy. No changes sync back to the Python reference.

Before a release update, close Atlas and back up the native `atlas.sqlite3` and any `legacy-recovery-*.sqlite3` files. Keep the Python reference database as a separate fallback until T026 owner acceptance and cutover are complete.

## Update, restart and uninstall

- **Restart:** close Atlas normally and reopen it from its Start menu shortcut or `%LOCALAPPDATA%\Atlas\atlas-native.exe`. Map edits commit to SQLite as they are made.
- **Update:** close Atlas and run the next per-user setup executable. NSIS updates application files at the registered install location; the data directory is separate. The T025 same-version update smoke left its synthetic native database byte-identical.
- **Uninstall:** use Windows Installed Apps or `%LOCALAPPDATA%\Atlas\uninstall.exe`. The interactive uninstaller offers a separate **Delete app data** checkbox; leave it clear to preserve `%APPDATA%\dev.atlas.workspace` and `%LOCALAPPDATA%\dev.atlas.workspace`. The T025 silent uninstall removed app files and its uninstall registration while leaving the external synthetic database intact. Back up data before uninstalling, especially if selecting the delete-data option.

T025 was installed, launched through the installed executable, updated, uninstalled, and reinstalled on Windows using synthetic database paths only. This did not migrate or open the owner's reference database. T026 remains responsible for owner-host acceptance, availability and a deliberate reversible cutover.
