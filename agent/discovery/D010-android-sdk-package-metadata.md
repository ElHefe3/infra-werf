# D010 — Match Android platforms by their package metadata

Date: 2026-10-25. State: verified, diagnostic corrected. Affected: T001/T002.

## Finding

The generated Android project requests integer API 37, but the stable SDK package is installed in `platforms/android-37.0` and reports `AndroidVersion.ApiLevel=37.0`. Directory-name assumptions would report a missing platform after a successful installation.

## Evidence

The resumed administrator build passed Tauri's native-library symbolic-link step and started Gradle. Gradle found existing accepted licences, installed Build Tools 36.0.0 and API 37.0 revision 2. The platform's `source.properties` reports API 37.0 and `PreviewSdkInt=0`. No new licence was accepted by automation and no Windows setting was changed.

## Consequence

Doctor now finds a matching platform through its metadata and `android.jar`, accepting numerically equivalent API 37 / 37.0. Installed-folder names alone are not the platform contract. Actual APK packaging is still a separate acceptance check.

## Follow-up

Verify the updated diagnostic in the build's final F01 checks, then record the actual APK result. Keep the successful SDK/Gradle/Cargo caches; do not reinitialize the project or reinstall tools to address a diagnostic path assumption.
