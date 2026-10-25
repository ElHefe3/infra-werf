# F01 — Reproducible native foundation acceptance

Date: 2026-10-25
Revision: 7176bbd plus the tested F01 foundation working tree (exact mise/package/Cargo pins, React/Tauri shells, generated Android project and diagnostic scripts).
Result: pass

## Tickets and acceptance

- **T001:** exact effective tools, truthful prerequisite diagnostics, dependency locks and real task mappings passed. [Toolchain evidence](F01-T001.md) records setup and negative checks. The final administrator-context `mise run test:feature -- F01` passed doctor, TypeScript checking and all tracker fixtures.
- **T002 Windows:** the x64 Tauri app built and launched. The owner confirmed **Rust connection verified** in its native window. The documented debug command ran again successfully with cached Rust dependencies; [Windows proof and historical build details](F01-T002-progress.md) remain recorded.
- **T002 Android:** `mise run smoke:android` produced a real APK, then ran successfully a second time using the existing Cargo/Gradle caches. The administrator helper completed with exit marker **0** and the final F01 checks passed. ARM64 Rust's Gradle rebuild reported 3.63s initially and 0.86s on the repeat. The complete resumed run, including first SDK/Gradle setup and checks, ran from 17:08:53 to 17:16:39 SAST.
- **T002 artifact inspection:** `aapt2 dump badging` reports package `dev.atlas.workspace`, version 0.1.0, minimum API 24, target/compile API 37 and native ABI `arm64-v8a`. ZIP/ELF inspection confirms the only packaged native library is ARM64 (`e_machine=183`). `apksigner verify --verbose` passed APK Signature Scheme v2 with one signer.

## Artifact and environment

APK: `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk` (133,134,308 bytes). SHA256: `93515F1D80FE4BD1E337A1D0AD32E65B1D89A66DD6D7151290C8D2180BA955A9`. The filename says universal, but its actual packaged ABI is ARM64 only. This is a debug development artifact; production signing and release-size optimisation are not claimed.

Tools remain Node 26.12.1, pnpm 12.15.0, Rust/Cargo 1.99.1 MSVC with Android targets, Python 3.15.1, Temurin Java 21.0.12.1+1, Tauri Rust 2.12.5 / CLI 2.12.4, Gradle 9.6.1 and AGP 9.3.1. Gradle installed SDK Platform 37.0 revision 2 and Build Tools 36.0.0 using licences already accepted on this PC; NDK 27.1.12297006 was reused.

## Repairs and bounded review

Owner-handled UAC allowed Tauri's required native-library symbolic link without changing Developer Mode or security policy. [D008](../discovery/D008-android-symlink-permission.md) records the original failure and resolution. [D010](../discovery/D010-android-sdk-package-metadata.md) records the doctor fix for the SDK's `android-37.0` directory and metadata. The final doctor reports both platform and symbolic-link readiness in the administrator build context; normal-user builds still need that permission.

Focused source/configuration and whitespace review covered the pinned setup, argument forwarding, capability/CSP boundary, helper privilege guard, ignored outputs and proof mapping. Rust formatting passed earlier and native source did not change. Tracker validation passed all ten rejection fixtures and the valid completion fixture. Independent code review was not performed; the owner supplied the Windows screen observation. Generated Android code emits nonfatal Java 8/deprecation warnings; its upstream source was not modified to suppress them.

## Limits and preservation

No emulator or physical-phone execution, release installer/signing, coordinator networking, authentication or map/storage migration is claimed. Actual Android interaction remains T026; release packaging remains T025. SDK command-line tools and the controlled SSH fixture are not configured, and doctor reports their status explicitly.

The Python reference and its database remain unchanged. Native artifacts/logs and machine configuration remain ignored; the tested Windows window was closed after proof. The administrator helper has finished; no build is left running. F01's two tickets are complete; S01's persistence/import gate still requires F02.
