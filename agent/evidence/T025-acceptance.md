# T025 — Windows desktop package and install proof

Date: 2026-10-27
Feature: F12
Ticket: T025
Revision: Application source at `7f9ba84`; the packaging harness used the working-tree `ATLAS_DESKTOP_BINARY` override to exercise the installed executable.
Platform: Windows x64, Tauri 2 / WebView2, current-user install
Result: pass
Distribution: unsigned; wider distribution needs approved code signing.

## Build configuration

`mise run build:desktop` produced the optimized NSIS installer using the project `mise.toml`, pnpm lockfile and Cargo lockfile. Tool versions resolved for the build were Node 26.12.1, pnpm 12.15.0, Rust 1.99.1 MSVC, Tauri CLI 2.12.4 and Tauri 2.12.5. Node 26.12.1 was the current release on the build date ([official Node release](https://nodejs.org/en/blog/release/v26.12.1/)). The lockfile supply-chain verification passed for all 108 entries. No dependency lockfile or application source changes were needed for packaging.

The Codex process redirects `LOCALAPPDATA` and the default mise cache. To use the existing pinned tools without reinstalling or changing global settings, this task set `MISE_DATA_DIR=C:\Users\owner\AppData\Local\mise`, `MISE_CACHE_DIR=.tools\mise-cache`, `MISE_AUTO_INSTALL=0`, `MISE_OFFLINE=1`, and explicitly selected the installed Rust 1.99.1 MSVC toolchain. Desktop requirements passed `mise run doctor`; Android symbolic-link permission is a separate S09 issue. See [D040](../discovery/D040-mise-data-root-in-codex-sandbox.md).

## Artifact identity

- Installer: `target/release/bundle/nsis/Atlas_0.1.0_x64-setup.exe`
- Size: 3,695,600 bytes
- Installer SHA-256: `C1FB8E1B73D85AA9CFA3CED4C4673CA59AF6CD5A5A74C1FE941C3915D559245A`
- Authenticode: `NotSigned`; no signer is embedded. SHA-256 detects changes but does not establish publisher identity.
- Release executable SHA-256 before NSIS metadata patch: `7C863E38A7642378728DB1516A2D42ED8D6F930BA8374D5B06CF751347CFBFC6`
- Installed executable SHA-256 after NSIS metadata patch: `D238E86959A8A9D65D2FF609361964D18F594AF7B14FB48C8FA17490037457ED`; Authenticode `NotSigned`.
- No signing-key/certificate files are tracked in Git. No certificate or signing secret was configured, requested, or added.

The NSIS script uses `currentUser` mode and `RequestExecutionLevel user`, installs by default below `%LOCALAPPDATA%\Atlas`, and uses the WebView2 download bootstrapper when the runtime is absent. That case requires internet access during install. The setup is a release package; `smoke:desktop` remains a debug build and is not described as distributable.

## Controlled installed-app proof

The generated setup was installed and the installed executable was used for `mise run test:desktop` with `ATLAS_DESKTOP_BINARY=C:\Users\owner\AppData\Local\Atlas\atlas-native.exe`. All nine result groups passed against a generated synthetic database; the original reference database was unchanged. Evidence: native journey result `.tools/desktop-proof-a94b7a13-8588-473d-814f-e3416d9d9d72/result.json` (local artifact, removed 2026-11-18; this paragraph is the retained record).

The installed-app journey covered:

- 1280×720 and 1440×900 desktop fit.
- 540-service inventory, nested search, keyboard shortcuts and inspector selection.
- Add/edit, duplicate rejection, directed relationships, drag, pan, zoom and persistence through reload/restart.
- Second-instance exit, workspace tabs, truthful protected-operation gating, startup retry and incompatible-database preservation.
- Zero renderer console errors; synthetic journey reported `legacyUnchanged: true`.

The same-version NSIS update completed successfully. The isolated synthetic database remained byte-identical (SHA-256 `F506D211DE1709F3D2EE9875C6B238FD7CAA6476CF170FF2663AB3087EBC2183`). Silent uninstall completed successfully, removed the app installation directory and uninstall registration, and left that external fixture database unchanged. The package was then reinstalled for T026 owner acceptance; the current-user release app remains installed.

The generated installer script was inspected for the data lifecycle. It stores app data separately under bundle ID `dev.atlas.workspace`; the interactive uninstaller offers a distinct **Delete app data** checkbox and only removes `%APPDATA%\dev.atlas.workspace` / `%LOCALAPPDATA%\dev.atlas.workspace` when selected. Update mode skips this deletion. The update/uninstall journey used `ATLAS_NATIVE_DB` pointed at the synthetic fixture, so it did not test deletion of real app data; no real application-data database was opened, migrated, or deleted. The registered install location was checked after NSIS ignored the attempted custom `/D` test destination. See [D041](../discovery/D041-tauri-nsis-user-data-lifecycle.md).

## Limits and recovery

- The package and executable are unsigned. Do not bypass Windows publisher or SmartScreen warnings. Obtain an approved certificate before wider distribution.
- Tauri's WebView2 bootstrapper needs network access only when a compatible runtime is missing.
- The installer does not import `data/atlas.sqlite3` automatically. First migration and reversible owner cutover remain T026; retain the Python reference and backups until the owner accepts.
- Packaging proof used synthetic data and did not contact any infrastructure host. Server 1 remained offline and untouched.
- Vite reported an existing bundle chunk above 500 kB; the release build succeeded. No docs-only edit triggered another build.

T025 acceptance criteria are met. T026 remains open for actual Windows-PC owner acceptance, approved-host checks and deliberate recovery/cutover decisions.
