# F01 / T002 — Native build progress and packaging boundary

Date: 2026-10-25
Revision: 7176bbd plus the F01 foundation working tree; source and pins described in BUILDING.md.
Result: partial — Windows proof passed; Android APK packaging awaits permission.

Historical checkpoint, superseded by [F01 acceptance](F01-acceptance.md): the owner resumed work, administrator packaging and its incremental rerun succeeded, and final F01 checks passed on 2026-10-25. The failed attempt below remains recorded for recovery context.

## Acceptance mapping

- T002 Windows build/launch: **passed**. `mise run smoke:desktop` compiled the locked Windows x64 native app in 4m32s. It launched as PID 19160 with a responding window titled `Atlas · Foundation`. The owner explicitly confirmed the displayed **Rust connection verified** result, proving the React-to-Rust command response in the actual application. UI automation could not initialize; no automated screenshot is claimed.
- T002 Android ARM64 app artifact: **incomplete**. `mise run smoke:android` compiled `target/aarch64-linux-android/debug/libatlas_native_lib.so` in 4m37s, then Windows denied Tauri's symbolic-link step before Gradle APK packaging. A compiled `.so` is not recorded as an installable Android application. Neither emulator nor physical-phone execution occurred.
- T002 reproducible commands/cache reuse: **partial**. The Windows command ran again successfully in 37.61s with the Rust dependency cache retained (only Atlas recompiled after the platform switch). pnpm and Gradle caches were retained; Gradle 9.6.1's version command passed under the selected Java 21. The corresponding successful APK rerun is still required.

## Artifacts and environment

Windows x64, pinned mise toolchain from [T001](F01-T001.md). Current debug executable SHA256 after the incremental rebuild: `1A67BFD80146FA2F814B6B039756EEE7C43CBC4BB56B9B0A5E34C579482ABD29`. The first executable observed by the owner had SHA256 `D1F82575E8FC2B5427C748C101C946CFB2B35CF958133792CD4207EA62E73D03`. These are separate build outputs; byte-identical reproducibility is not claimed.

Compiled ARM64 library SHA256: `9B47148D2DA7670B13AB03A4B6BD49EF074D039153ACF72BFADACCF28ADBD99E`. Native artifacts stay under ignored build output directories, not in Git.

## Checks and repairs

- Frontend type checking and Vite production bundling passed in both platform builds.
- Rust formatting check passed. A PowerShell advanced-parameter binding issue initially consumed Cargo's `--` separator; the wrapper now uses plain argument forwarding and the same command passed.
- Real tracker validation, all ten rejection fixtures and the valid completion fixture passed after fixing live-state-dependent self-test preconditions; [D009](../discovery/D009-tracker-fixture-state.md).
- Doctor correctly returned 2 for missing Android API 37 and denied native symbolic-link creation. Its temporary workspace probe left zero directories behind. Release signing, phone acceptance and the controlled SSH fixture remain explicitly unverified.
- Diff whitespace review passed. No independent code review or Android runtime journey is claimed.

## Blocker and recovery

See [D008](../discovery/D008-android-symlink-permission.md). A narrowly scoped administrator helper was prepared to run the APK build and one incremental rerun. Windows cancelled or timed out its UAC request before the helper ran. On 2026-10-25 the owner answered the relaunch question with **Record Android packaging as blocked for now**. No further UAC request or packaging retry is authorised until the owner asks to resume. No Developer Mode/security-policy change or new Android licence acceptance was performed.

Retain the blocker and existing caches. On a future explicit resumption: owner-handled UAC, then run `scripts/android-smoke-admin.ps1` in an administrator context; inspect ignored `.tools/logs/android-smoke.log` and `.exit`. It stops on the first failed build. API 37/licence readiness remains an additional unverified packaging prerequisite.

## Cleanup / scope limit

The verified Windows test window was closed for its incremental rebuild. The legacy Python application/database were not changed. This foundation does not yet migrate the map, storage, connections or operations. T002 remains incomplete; F02 cannot start until F01's native gate passes. Release installers, signing and actual phone acceptance belong to later tickets.
