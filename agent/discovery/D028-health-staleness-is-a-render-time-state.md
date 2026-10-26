# D028 — Health staleness is derived from observation age at render time

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T019, T026

## Finding

The Health UI stores only the probe configuration in local storage. Health observations live in Rust memory and carry `observed_at`. The renderer labels a report stale when its observation is more than 15 minutes old; that label is derived at render time. A native UI test can verify this without waiting 15 minutes by using Playwright's fixed browser clock while leaving timers and the Rust system clock unchanged.

## Evidence

On 2026-10-26, `scripts/verify_native_health.mjs` ran inside the actual Windows Tauri/WebView2 app with two loopback targets. It advanced only the renderer's `Date` after a healthy report and observed the stale label, then restored wall time and proved reachable-host/unhealthy-service and closed-endpoint states. The final native result is `.tools/native-health-proof-35c0d635-5943-4ee3-abce-8327cb4e8049/result.json`; it also verifies the exact successful service timestamp remains unchanged after HTTP 503. The map database hash remained unchanged and no page errors occurred.

## Consequence

Keep the source timestamp authoritative and derive freshness from the observation age; do not persist a second stale flag or convert an old healthy result into a failed probe. UI tests can use a fixed renderer clock to check aging, while service state transitions still require actual loopback probe results.

## Follow-up

T026 release proof should retain the distinction between old observations and failed checks. Mobile should reuse the timestamped contract while implementing its own native polling adapter in future S09.
