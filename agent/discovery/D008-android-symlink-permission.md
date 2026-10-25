# D008 — Android packaging needs symbolic-link permission on this PC

Date: 2026-10-25. State: resolved for approved administrator builds; ordinary-user permission is still required. Affected: T002.

## Finding

The shared frontend and Android ARM64 Rust library compile with the pinned toolchain. Tauri then creates a symbolic link from the compiled `.so` into the generated Android JNI library directory. Windows denied that operation in the normal-user build context.

## Evidence

`mise run smoke:android` completed Rust's dev profile in 4m37s and produced `target/aarch64-linux-android/debug/libatlas_native_lib.so`. Tauri reported `Creation symbolic link is not allowed for this system` before Gradle packaging. No APK was produced. Tauri's error names Developer Mode or symbolic-link privilege as prerequisites; no system setting was changed.

The bounded `scripts/android-smoke-admin.ps1` helper runs the same smoke task, then one incremental rerun only if it passes. An administrator launch was requested through the real approval mechanism, but Windows returned `The operation was canceled by the user` before the helper created a transcript. That message does not establish whether the human intentionally cancelled or the UAC prompt timed out. On 2026-10-25 the owner explicitly chose **Record Android packaging as blocked for now** when asked about relaunching.

## Consequence

Windows native compilation/launch is proven; Android Rust compilation is proven; Android APK packaging remains unproven. API 37 and its package/licence readiness have not yet been reached. Keep T002 incomplete and do not advance into F02 while this gate is unresolved.

## Follow-up

The owner explicitly resumed work. The approved administrator helper created the link, built the ARM64 APK twice and passed final F01 checks with exit 0; see [F01 acceptance](../evidence/F01-acceptance.md). No Developer Mode/security-policy setting changed. Future Android builds still need a context permitted to create symbolic links; preserve caches and use actual UAC approval when necessary. Do not autoaccept new Android licences.
