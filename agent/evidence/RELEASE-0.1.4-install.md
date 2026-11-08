# InfraWerf 0.1.4 owner install

Date: 2026-11-08. Revision: `608c572` plus the 0.1.4 version bump committed with this record. Owner request: "Install it".

- Installer: `mise run build:desktop` → `target/release/bundle/nsis/InfraWerf_0.1.4_x64-setup.exe`, SHA-256 `8603F050F0320A520CA4F56AEC673072EFAF5072E83978212ED05D7612130014` (3.65 MiB, unsigned). The NSIS toolchain download returned HTTP 504, so the previously cached toolchain was copied from the Codex package cache to `%LOCALAPPDATA%\tauri\NSIS`.
- Installed silently (`/S`, current user) from an unpackaged shell. Registration: DisplayVersion 0.1.4, InstallLocation `%LOCALAPPDATA%\InfraWerf`; Start Menu shortcut `InfraWerf.lnk`. Installed executable SHA-256 `4ADD1BF0C1E93C751E1498CF870E225D76E9FC97BD19E4A49F5D5028010760FB` (differs from the build-tree binary because of the NSIS bundle-type patch, as in T025).
- `mise run start:desktop` now finds the registration and opened the installed 0.1.4 executable; it holds the workspace store's exclusive lock, i.e. startup opened the owner store (schema 3, no migration).
- Pre-install backup: `%APPDATA%\dev.atlas.workspace\pre-0.1.4-install-20261108-*` with `atlas.sqlite3` SHA-256 `88FECA52A2FFF367163D22F8D9D102B33029FFB1A834A7B3B547A3C1B33830C4` and its journal.
- The old 0.1.3 program in the Codex package cache (D050) was left in place, unused; removing it is an owner decision.

Result: installed and launched. Visual confirmation of the owner's map and the Apps tab is left to the owner.
