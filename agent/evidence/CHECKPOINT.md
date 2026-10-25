# Atlas restart checkpoint

Date: 2026-10-25
State: S01 in progress; F01/T001–T002 done; F02/T003 ready.
Revision: F01 foundation working tree based on 7176bbd; [F01 acceptance](F01-acceptance.md) identifies the tested source/pins and artifacts. Record the resulting local milestone commit when starting F02.

## Working set

Read [AGENT.md](../AGENT.md), [S01](../scopes/S01-foundations.md), then [F02](../features/F02-domain-persistence.md). Next: **T003**, then T004/T005. [F01 acceptance](F01-acceptance.md) closes the native-build prerequisite. The owner authorised S01 implementation and resumed work; no repeat conversational approval is needed for routine work within that scope.

Core: native Windows/Android, PC-hosted, full phone controls, LAN/existing VPN. Agents/rsync deferred. Project pins/tasks and both dependency locks now exist; see [BUILDING.md](../../BUILDING.md). Preserve the Python reference/database and unrelated work; the thin native shell does not access the reference database.

## Verification and blockers

Documentation evidence is [DOCS-001](DOCS-001.md); all ten rejection / one positive tracker fixtures passed after real completion. Counts: 31 tickets / 14 features / 8 scopes; done=2, ready=1, todo=23, deferred=5. Effective tools: Node 26.12.1, pnpm 12.15.0, Rust/Cargo 1.99.1 MSVC, Python 3.15.1, Java 21.0.12.1+1; mise 2026.9.1. Doctor finds MSVC 14.44.35207/linker, Windows SDK and WebView2.

New native setup findings are [D007](../discovery/D007-native-setup-boundaries.md). Mise task wrappers explicitly prepend selected tool bins because inherited child PATH found Node 14 first. Sandbox redirects application-data paths; normal-user tools/processes can appear missing inside it. Use real escalation for required commands, without changing sandbox policy. D005's earlier edit-rejection cause remains unproven; D006's separate SSH-host error was not changed.

## Next commands

Verified: `mise run deps`, `tracker:check`, `test:feature -- F01`, both `smoke:desktop` / `smoke:android` commands and their cache reuse, frontend checks, Rust formatting, task listing and explicit nonzero absent-suite checks. Use `mise run exec -- ...` for one-offs. Doctor passed all required checks in the approved administrator build context. Ordinary-user Android builds still need symbolic-link permission. SDK Platform 37.0 revision 2 and Build Tools 36.0.0 were installed using already accepted licences; NDK 27.1.12297006 was reused. Doctor matches SDK metadata ([D010](../discovery/D010-android-sdk-package-metadata.md)). Release signing/device/SSH fixture remain unverified.

## Completed native proof

- `mise run smoke:desktop` passed in 4m32s: `target/debug/atlas-native.exe`; SHA256 `D1F82575E8FC2B5427C748C101C946CFB2B35CF958133792CD4207EA62E73D03`.
- App launched; normal-user process inspection showed PID 19160 responding, title `Atlas · Foundation`. The owner explicitly confirmed the displayed **Rust connection verified** status. Sandboxed process inspection could not see it; that was not an app exit.
- Automated UI inspection was unavailable: node_repl failed to write kernel assets after one reset/retry; unified connector initialization timed out. No automated screenshot or independent review is claimed; owner observation supplies screen proof.
- Windows command reran successfully in 37.61s with dependency caches retained; its test window was closed for that rebuild. [T002 progress](F01-T002-progress.md) records current artifact hashes.
- Android packaging and its repeat build passed after owner-handled UAC. The helper finished with exit **0**, including final F01 doctor/type/tracker checks. APK: `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`, 133,134,308 bytes, SHA256 `93515F1D80FE4BD1E337A1D0AD32E65B1D89A66DD6D7151290C8D2180BA955A9`. Signature verification and ZIP/ELF inspection passed; ARM64 only, min API 24 / target API 37. No emulator/phone execution or production signing is claimed. No build remains running.

Owner decision (2026-10-25): the earlier hold was superseded by **lets resume**; the prepared recovery succeeded. Preserve caches. Do not autochange Developer Mode/security policy or accept new licences. [D008](../discovery/D008-android-symlink-permission.md) retains the resolved permission history.

Next: F02 shared contracts, native-owned SQLite and classified/atomic imports. Relevant source `atlas.py` and `tests/test_atlas.py` was read once during the build; port meaningful cases, not the legacy unauthenticated LAN API. Use synthetic fixtures and a safe SQLite backup/copy; never cut over or co-write the live reference. Finish F02's parity/rollback/classification gate before S02. Signing and real-phone acceptance remain T025/T026.
