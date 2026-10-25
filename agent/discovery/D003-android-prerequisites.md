# D003 — Android SDK presence is not native build proof

Date: 2026-10-25
State: verified
Affected: T001, T002, T009, T025, T026

## Finding

SDK platforms `android-35`/`android-36`, build-tools `35.0.0`/`36.1.0`, NDK `27.1.12297006` and Android Studio are present. The selected Tauri/Gradle/JDK/SDK/NDK compatibility chain, Android Rust targets, signing and device execution remain unverified.

## Evidence

Read-only SDK/NDK/installation directory inspection during planning. WebView2 application directory `155.0.4283.62` was observed too. No licence acceptance, Android initialisation, dependency installation or native compilation occurred.

## Consequence

A phone-size browser and installed SDK folders cannot close native Android acceptance. Resolve latest stable compatible requirements in T001 and compile thin targets in T002.

## Follow-up

T009 verifies native adapters, T025 creates the signed APK and T026 requires physical-phone acceptance. Observed SDK versions are not pins. Sources: [Tauri setup](https://v2.tauri.app/start/prerequisites/), [mobile development](https://v2.tauri.app/develop/).
