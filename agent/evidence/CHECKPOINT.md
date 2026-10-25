# Atlas restart checkpoint

Date: 2026-10-25
State: **S01 complete; S02 desktop map/lifecycle (T006–T008) and operation guard foundation (T032) passed**. T009 phone composition/adapters is ready; S02 stays open. Owner priority remains **desktop first**.
Revision: desktop map **e6760a3**. T032 tested working tree is based on it; latest local completion commit is visible in `git log`. [T032 proof](T032-acceptance.md), [F03 proof](F03-acceptance.md), [Windows proof](F04-desktop-acceptance.md).

## Resume with this working set

Read [AGENT.md](../AGENT.md), [S02](../scopes/S02-applications-map.md), and the relevant next feature packet. Do not reread the full transcript or redo completed setup. F03 desktop and T008 passed; phone/touch requirements moved intact to T009. Core remains Windows/Android, Windows coordinator, full phone controls and LAN/existing VPN. Agents/rsync remain deferred.

For desktop SSH continuation, read [D014](../discovery/D014-desktop-guard-phasing.md) and [OPERATIONS.md](../OPERATIONS.md), then stage one actual native SSH trust/authentication journey. T014 still includes real paired transport/executor/mutation proof; T012 depends on it. Do not mark those passed because T032's controlled library tests passed. The tracker scope-completion rules were preserved. No controlled SSH server fixture is established yet; Windows OpenSSH 9.5p2 exists, Docker daemon was unavailable. No live identities/trust files were read or changed. Reuse the guard rather than rebuilding it or repeating unrelated UI/native setup.

The Windows app now contains the real React Flow map, inspector, full saved inventory and workspace tabs, backed by native Rust/SQLite. Open **Atlas Desktop.cmd** or `mise run start:desktop`; no Python/dev server is needed at runtime. The original Python database stays untouched and recoverable. The native store is a separate one-time copy, not synchronised with Python. Existing Android APK remains the foundation composition; compact phone UI/adapters and physical execution are unproven. Remote coordinator/SSH/files/terminal/transfers remain unavailable.

## Completed scope and proof

- [F01](F01-acceptance.md): exact mise tools/tasks, Windows app build/launch with owner-observed Rust bridge, real ARM64 debug APK, signature/ABI inspection and repeat-build cache proof.
- APK: `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`; ARM64 only despite the universal filename, 133,134,308 bytes, SHA256 `93515F1D80FE4BD1E337A1D0AD32E65B1D89A66DD6D7151290C8D2180BA955A9`. This is the F01 foundation artifact, not a map release. No emulator/phone execution or production signing is claimed.
- [F02](F02-acceptance.md): one Rust/generated-TypeScript contract, native-owned SQLite, safe-copy migration/recovery and bounded atomic classified imports. Final **16 Rust + 2 adapter tests**, integration journey, binding/type/fmt/Windows checks and Android ARM64 core compilation passed.
- Real-reference proof used a read-only backup: 4 servers / 0 edges / 4 events; stable owner data, rollback and actual separate Python-writer rejection passed. Original digest unchanged. Final private copies: ignored `.tools/data-proof-f7957dc76b59438182024c7a9bf7086f/`. Do not dump their inventory or add them to Git.
- Independent review found schema-prefix/WAL-size guard defects; both were fixed, regressed and confirmed resolved. [D012](../discovery/D012-snapshot-review-guards.md). No running proof child remains.
- [F03](F03-acceptance.md)/[T008](F04-desktop-acceptance.md): actual WebView2/Rust IPC save/reload/native Quit/relaunch, directed links, full inventory, pan/zoom, keyboard selection/move, duplicate-host/error recovery, two laptop viewports and single instance passed. Final synthetic proof: `.tools/desktop-proof-69031c5b-2947-465a-b4cf-78e665bd5a6c/`. Independent review's two keyboard findings fixed and confirmed resolved.
- Real owner native copy: **4 servers / 540 saved workloads**, exact IDs/names/hosts/positions and original digest preserved. Private `.tools/owner-desktop-proof-498139e70c744bcb9d7c8eeb0c7886c5/result.json`. Technical proof, not owner release acceptance. No host connection or live operation.
- Current checks: **18 Rust + 4 client/model tests**, actual native desktop journey, `mise run check`, tracker self-tests and desktop feature mapping passed. No Android rebuild for this desktop-only change.
- [T032](T032-acceptance.md): final **27 core tests** passed (11 guard tests including reviewed cancellation/entry races + 16 existing regressions), plus shared contract/type/fmt/Windows/Android-core checks. No UI/APK rebuild for a library-only foundation. No SSH/network executor or new protected-operation IPC is enabled. Record actual child termination, resolved endpoint/key binding, native lease lifetime and canonical-path/symlink race proof in the integration tickets.
- Tracker: 32 tickets / 14 features / 8 scopes; 27 core / 5 deferred. T032 extracts T014's desktop guard, without adding product scope. States: done=9, ready=1 (T009), todo=17, deferred=5. S01 complete; combined S02 Windows/phone gates remain open pending T009.

## Effective tools and commands

Pins: Node 26.12.1, pnpm 12.15.0, Rust/Cargo 1.99.1 MSVC, Python 3.15.1, Java runtime 21.0.12.1+1; mise 2026.9.1. Tauri Rust 2.12.5 / CLI 2.12.4; Gradle 9.6.1 / AGP 9.3.1; SDK 37.0 revision 2, Build Tools 36.0.0, NDK 27.1.12297006. Contracts ts-rs 12.0.1; bundled SQLite rusqlite 0.41.0; tempfile 3.27.0. Keep locks/caches; no mid-feature upgrades.

Verified: `mise run deps`, `check`, `tracker:check`, `contracts:generate` / `contracts:check`, `test:feature -- F01` / `F02` / `F03`, `test:desktop`, `test:integration`, `check:android-core`, both native smoke builds and repeat cache checks. Desktop start: `mise run start:desktop`. One-off child commands use `mise run exec -- ...`; wrapper prepends selected bins and forwards Cargo's separator correctly. Unknown feature tests fail explicitly. New exact dependencies: single-instance 2.5.4, Playwright test 1.64.1.

Android packaging needs owner-handled administrator UAC for symbolic links on this PC; the prepared helper succeeded after resumption. Do not autochange Developer Mode/security policy. Core cross-checks need no UAC and use the configured NDK directly. SDK licences were already accepted; no new licences were autoaccepted. Doctor matches platform metadata, including the `android-37.0` directory. Release signing, physical device and SSH fixture remain explicitly unverified. [BUILDING.md](../../BUILDING.md) has commands.

## Data and interface boundaries for S02

Read [CONTRACTS.md](../CONTRACTS.md), [INVENTORY.md](../INVENTORY.md) and [STORAGE.md](../STORAGE.md) as needed. `crates/atlas-core` is the shared domain; `ui/contracts.gen.ts` is generated, not hand-edited. Native status and typed coordinator adapter consume it; real authenticated transport is F05.

Use a separate native-owned database, never concurrent Python/Rust writes to the reference. Migration/restore create new destinations, retain recovery and reject unsupported data/schema. Configuration/raw/warning fields are excluded from overview; saved state/freshness never implies live reachability or service health. Real UI wiring/cutover still needs its own acceptance.

Persistent Node REPL still fails before initialisation, but the CLI Playwright harness now drives the **actual native WebView2** using process-local loopback CDP and isolated synthetic data ([D013](../discovery/D013-native-desktop-proof.md)). It passed the desktop save/reload/restart/keyboard/layout/error journey. Debugging is disabled in the normal owner launcher. T009 still needs compact-phone/touch/native-adapter proof; physical-phone acceptance stays T026. Signing, release cutover and owner/device gates remain T024–T026. Before F05/F08/F09 enable remote work, register it for its full lifetime with the shared native close guard and test visible close/quit warnings.
