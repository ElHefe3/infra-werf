# D038 — Wait for the Windows WebView, not the Tauri launcher handle

Date: 2026-10-26<br>
Affected: T024

## Finding

On Windows, the process handle returned by launching the debug Tauri executable can exit successfully while the native Atlas/WebView2 window continues running. Treating that zero exit code as application termination made the desktop proof fail before checking its loopback CDP endpoint. A pre-existing instance can also receive a single-instance handoff without inheriting a new test's WebView debug settings.

## Resolution

The native proof now waits for the expected loopback WebView endpoint to become available and, after the app's normal Quit action, unavailable. It checks that Atlas and the chosen port are free before starting. The exact diagnostic Atlas window started with a synthetic database was closed through its normal window-close request. No process was force-terminated.

## Consequence

For Windows Tauri UI tests, the launcher process exit code is not a reliable signal for native-window readiness or closure. Observe the test-owned WebView endpoint and use a fresh synthetic profile; do not infer ownership from an exit code or terminate an existing Atlas instance.
