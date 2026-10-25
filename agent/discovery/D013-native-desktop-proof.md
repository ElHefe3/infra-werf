# D013 — Desktop-first delivery and actual WebView2 proof

Date: 2026-10-25. Related: S02, T006–T009.
State: **verified**.

## Finding

The owner explicitly prioritised a working desktop. F03's compact-phone composition, touch and 320/390px requirements moved intact to T009; the S02/core phone gates stay open. Ticket counts and scope did not shrink.

The persistent Node REPL still fails before initialisation (`failed to write kernel assets`, missing path). A normal-user automated CLI journey can nevertheless drive **the actual Tauri WebView2** through Playwright CDP with process-local `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`. `mise run test:desktop` launches only synthetic data, binds temporary debugging to `127.0.0.1:9225`, isolates WebView data under ignored `.tools/`, and stops its child. This does not require changing sandbox/global settings or using a mock backend. The owner launcher removes the debug argument environment variable.

DOM `window.close()` does not request native Tauri close in this WebView2. Tauri's window close API does; a visible Quit action uses that API and exercises the application's close guard. React Flow's initial fit also needs measured node dimensions to avoid clipped bounds.

## Evidence

`mise run test:desktop` passed the full native journey on 2026-10-25; final private result is `.tools/desktop-proof-69031c5b-2947-465a-b4cf-78e665bd5a6c/result.json`. This includes keyboard selection/move persistence after review fixes. See [F03](../evidence/F03-acceptance.md).

## Consequence

Use the native journey for desktop persistence/layout/restart proof, reusing the built binary. Do not equate it with Android/touch/physical-owner acceptance. Keep native and Python databases separate: the reference remains read-only during first import, and subsequent native edits do not sync back. Authenticated remote work must register with the lifecycle guard before its feature is enabled.

Primary references: [Playwright WebView2](https://github.com/microsoft/playwright/blob/main/docs/src/webview2.md), [Microsoft WebView2 debugging](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code), [Tauri single instance](https://tauri.app/plugin/single-instance/).

## Follow-up

T009 retains all compact-phone/touch proof. F05/F08/F09 must register actual remote/background work with the close guard and verify the corresponding user warnings. Keep debugging process-local and disabled in the owner launcher.
