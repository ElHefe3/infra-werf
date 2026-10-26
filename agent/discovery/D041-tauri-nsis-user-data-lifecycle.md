# D041 — Tauri NSIS separates per-user install files from app data

Date: 2026-10-27<br>
State: verified<br>
Affected: T025, T026

## Finding

The generated Tauri 2.12 NSIS script uses `currentUser` mode (`RequestExecutionLevel user`), installs under `%LOCALAPPDATA%\Atlas`, and stores application data separately under the bundle identifier `dev.atlas.workspace`. Update mode skips the uninstaller's optional app-data deletion. The interactive uninstaller exposes a **Delete app data** checkbox; removal of `%APPDATA%\dev.atlas.workspace` and `%LOCALAPPDATA%\dev.atlas.workspace` occurs only when that option is selected.

## Evidence

The T025 setup was installed, launched through its installed executable, updated, silently uninstalled, and reinstalled. Update returned success and left the generated native SQLite database byte-identical. Uninstall removed the program directory and HKCU registration while preserving the fixture database. The custom `/D` test argument did not select the expected workspace folder; the registered install location was checked directly and matched the documented per-user default.

## Consequence

Treat the install directory and app-data directory as separate backup/recovery concerns. Verify the NSIS-registered path instead of assuming a custom `/D` destination. T026 must preserve the user's native data and Python fallback until explicit owner cutover.
