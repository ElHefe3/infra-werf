# Atlas restart checkpoint

Date: 2026-10-25
State: **S01 complete** (T001–T005). Next planned batch: S02/F03/T006, ready.
Revision: F01 milestone **23663d2**. F02's tested working tree and final guards are recorded in [F02 acceptance](F02-acceptance.md); its local completion commit is visible in `git log`.

## Resume with this working set

Read [AGENT.md](../AGENT.md), [S02](../scopes/S02-applications-map.md), then [F03](../features/F03-react-map.md). T006 is ready; T007 follows it, then F04 native shells. Do not reread the full transcript or redo completed setup. Core remains Windows/Android, Windows coordinator, full phone controls and LAN/existing VPN. Agents/rsync remain deferred.

The native window/APK still uses the foundation composition. The real React Flow desktop/compact-phone UI is **not implemented yet**. Preserve approved appearance and real identities/layout. The Python UI/database remain the working reference; no live cutover occurred.

## Completed scope and proof

- [F01](F01-acceptance.md): exact mise tools/tasks, Windows app build/launch with owner-observed Rust bridge, real ARM64 debug APK, signature/ABI inspection and repeat-build cache proof.
- APK: `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`; ARM64 only despite the universal filename, 133,134,308 bytes, SHA256 `93515F1D80FE4BD1E337A1D0AD32E65B1D89A66DD6D7151290C8D2180BA955A9`. This is the F01 foundation artifact, not a map release. No emulator/phone execution or production signing is claimed.
- [F02](F02-acceptance.md): one Rust/generated-TypeScript contract, native-owned SQLite, safe-copy migration/recovery and bounded atomic classified imports. Final **16 Rust + 2 adapter tests**, integration journey, binding/type/fmt/Windows checks and Android ARM64 core compilation passed.
- Real-reference proof used a read-only backup: 4 servers / 0 edges / 4 events; stable owner data, rollback and actual separate Python-writer rejection passed. Original digest unchanged. Final private copies: ignored `.tools/data-proof-f7957dc76b59438182024c7a9bf7086f/`. Do not dump their inventory or add them to Git.
- Independent review found schema-prefix/WAL-size guard defects; both were fixed, regressed and confirmed resolved. [D012](../discovery/D012-snapshot-review-guards.md). No running proof child remains.
- Tracker: 31 tickets / 14 features / 8 scopes; 26 core / 5 deferred. States after S01 closure: done=5, ready=1, todo=20, deferred=5. Scope S01's gate and evidence are complete.

## Effective tools and commands

Pins: Node 26.12.1, pnpm 12.15.0, Rust/Cargo 1.99.1 MSVC, Python 3.15.1, Java runtime 21.0.12.1+1; mise 2026.9.1. Tauri Rust 2.12.5 / CLI 2.12.4; Gradle 9.6.1 / AGP 9.3.1; SDK 37.0 revision 2, Build Tools 36.0.0, NDK 27.1.12297006. Contracts ts-rs 12.0.1; bundled SQLite rusqlite 0.41.0; tempfile 3.27.0. Keep locks/caches; no mid-feature upgrades.

Verified: `mise run deps`, `check`, `tracker:check`, `contracts:generate` / `contracts:check`, `test:feature -- F01` / `F02`, `test:integration`, `check:android-core`, both native smoke builds and repeat cache checks. One-off child commands use `mise run exec -- ...`; wrapper prepends selected bins and forwards Cargo's separator correctly. Unknown feature tests fail explicitly.

Android packaging needs owner-handled administrator UAC for symbolic links on this PC; the prepared helper succeeded after resumption. Do not autochange Developer Mode/security policy. Core cross-checks need no UAC and use the configured NDK directly. SDK licences were already accepted; no new licences were autoaccepted. Doctor matches platform metadata, including the `android-37.0` directory. Release signing, physical device and SSH fixture remain explicitly unverified. [BUILDING.md](../../BUILDING.md) has commands.

## Data and interface boundaries for S02

Read [CONTRACTS.md](../CONTRACTS.md), [INVENTORY.md](../INVENTORY.md) and [STORAGE.md](../STORAGE.md) as needed. `crates/atlas-core` is the shared domain; `ui/contracts.gen.ts` is generated, not hand-edited. Native status and typed coordinator adapter consume it; real authenticated transport is F05.

Use a separate native-owned database, never concurrent Python/Rust writes to the reference. Migration/restore create new destinations, retain recovery and reject unsupported data/schema. Configuration/raw/warning fields are excluded from overview; saved state/freshness never implies live reachability or service health. Real UI wiring/cutover still needs its own acceptance.

F01 automated UI tools failed to initialise; owner observation verified the Windows screen. That historical failure is not a permanent capability claim. S02 must obtain its own real save/reload and touch-layout proof, with physical-phone acceptance reserved for T026. Signing, release cutover and owner/device gates remain T024–T026.
