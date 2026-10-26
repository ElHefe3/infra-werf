# T019 — Bounded HTTP/TCP health checks

Date: 2026-10-26

Revision: 7c2e941 + T019 reviewed working tree (uncommitted)

Result: pass

## Included slice and environment

Windows x64 Tauri 2.12.5/WebView2; Rust 1.99.1 MSVC; Node 26.12.1 and pnpm 12.15.0. The app used a fresh temporary SQLite database and disposable loopback-only TCP/HTTP endpoints. No real server was probed. Health results stayed in Rust memory; only the per-server probe configuration was temporarily stored in the renderer and it was removed by Stop checks.

## Acceptance mapping

- **Independent and truthful states:** The native Health tab first displayed separate unknown host/service states. A live loopback TCP host and HTTP 200 service then showed reachable/healthy with real checked/success timestamps. With the host still reachable, a real HTTP 503 changed only the service to unhealthy and recorded a last-failure time. Both the exact Rust `last_success_at` and rendered Last success label remained unchanged across the failure. Closing the service listener kept host reachable while the endpoint stayed unhealthy.
- **Stale and missing observations:** The native harness moved only the renderer's fixed `Date` forward by 16 minutes after a real healthy report; the displayed observation changed to Stale. It then restored browser time. Stop checks removed the configuration and report and returned both states to unknown/no observation. See [D028](../discovery/D028-health-staleness-is-a-render-time-state.md).
- **Bounds, cancellation and validation:** The existing focused Rust fixtures cover HTTP/TCP outcomes, unreachable endpoints, endpoint/path/port validation, timeout, backoff, and cancellation during a stalled read. The native Stop checks action cancelled monitoring and removed its local configuration.

## Executed proof

- `mise run test:feature -- T019`: six focused Rust tests and four UI/client model tests passed; generated contracts and tracker checks passed.
- `cargo check --workspace --locked`, contract export check, `pnpm check`, and `cargo fmt --all -- --check` passed. `pnpm check` was rerun after the active/draft result-label fix.
- `node --check scripts/verify_native_health.mjs`: passed.
- `pnpm tauri build --debug --no-bundle -- --locked`: rebuilt `target/debug/atlas-native.exe` successfully with the active/draft result-label fix.
- `scripts/verify_native_health.mjs`: actual Windows Tauri/WebView2 journey passed all seven results in `.tools/native-health-proof-35c0d635-5943-4ee3-abce-8327cb4e8049/result.json`; zero WebView page errors. It verifies timestamp values are real and the exact success timestamp and displayed label persist unchanged after HTTP 503. The harness confirmed the map's server/edge/event records and SQLite byte hash were unchanged. The top-level `served_at` response timestamp was intentionally excluded from object equality because it changes on every read.
- Test-task wiring is available as `mise run test:health-native` for a future native rerun.
- Independent bounded review: **Pass**; both the active/draft endpoint-label finding and timestamp evidence gap are closed.

## Refinement and limits

Initial harness assertions were corrected for the no-observation guidance text and fresh `map_state.served_at`. Review also led to keeping saved result endpoint labels tied to the active probe while editing a draft, and to asserting the exact prior success timestamp survives HTTP 503. These refinements are covered by the final native run. Staleness is verified by aging the renderer clock, not by waiting 15 real minutes or changing the Rust clock. The service HTTP failure and closed endpoint are real loopback probes.

The app tests plain unauthenticated HTTP GET only, with no redirects, credentials, Internet scanning or persisted probe result. This does not claim Android polling or physical-device proof; mobile remains future S09. Owner endpoints were not contacted.

## Cleanup and rollback

The native app exited through Quit Atlas. Both loopback listeners closed in the harness. The temporary database, WebView profile and proof remain in ignored `.tools/native-health-proof-35c0d635-5943-4ee3-abce-8327cb4e8049/`. Rollback is to revert the T019 evidence/test-task additions; no user data migration is involved.
