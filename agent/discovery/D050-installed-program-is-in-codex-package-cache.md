# D050 — The installed program lives in a Codex package cache

Date: 2026-11-07. Related: F16 / T034, D048.

The owner's running InfraWerf was `%LOCALAPPDATA%\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\InfraWerf\atlas-native.exe`. The installer apparently ran inside the packaged Codex app, so Windows redirected the program folder (and probably its `HKCU` uninstall registration) into that package. Outside Codex, `%LOCALAPPDATA%\InfraWerf` and the registration do not exist, so `mise run start:desktop` fails with "Install InfraWerf first".

Workspace data is not redirected: the store is the ordinary `%APPDATA%\dev.atlas.workspace\atlas.sqlite3`, beside a `pre-monitoring-*.sqlite3` schema-2 recovery copy. The program launched directly from the package path opens that normal store.

This likely explains part of D048's unresolved install/location discrepancy. Reinstalling from an unpackaged shell would put the program in the ordinary location; that is an owner decision and was not done. The tauri single-instance plugin is unconditional, so native fixture journeys require the owner app to be closed first.

Update 2026-11-08: 0.1.4 was installed from an unpackaged shell to the ordinary `%LOCALAPPDATA%\InfraWerf` with its registration, so `start:desktop` works again ([install record](../evidence/RELEASE-0.1.4-install.md)). The Tauri NSIS toolchain cache had the same redirection and was copied to `%LOCALAPPDATA%\tauri\NSIS`.
