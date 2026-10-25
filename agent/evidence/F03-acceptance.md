# F03 desktop map acceptance

Date: 2026-10-25
Revision: desktop working tree based on **0145d7a**; final local commit recorded by Git/checkpoint after review.
Result: pass
Tickets: **T006, T007** (desktop-first acceptance). Compact-phone requirements were moved intact to T009 by the owner's explicit priority; S02 remains open.

## Environment and proof

Windows x64, Tauri 2.12.5 / CLI 2.12.4, WebView2, React 19.3.1, React Flow 12.12.0. Pinned mise Node 26.12.1, pnpm 12.15.0 and Rust 1.99.1 MSVC. New exact dependencies: Tauri single-instance 2.5.4 and Playwright test 1.64.1. No global runtime changes.

`mise run smoke:desktop` built the actual standalone debug app. `mise run test:desktop` passed against its **real WebView2 and Rust IPC**, using a private synthetic legacy database imported on first launch. Four hosts, 556 saved workloads, including one 540-service report; synthetic inputs only. This is not a mocked browser backend.

Final private artifacts: `.tools/desktop-proof-69031c5b-2947-465a-b4cf-78e665bd5a6c/` contains `result.json`, 1280×720 and 1440×900 screenshots, final desktop and startup-error screenshots, synthetic source/native/recovery databases and isolated WebView profile. Do not commit them. Earlier passing run `desktop-proof-afb97386-fd46-473d-8fbf-51d990ca947f` predates review fixes and is superseded.

## Acceptance mapping

- **T006 persistence:** add host, rename, drag node, create directed labelled relationship through UI/Rust IPC. Renderer reload and native Quit/relaunch preserved IDs, edited fields, exact coordinates and relationships. Explicit edge removal persisted after another reload.
- **T006 map mechanics/identity:** host and deep workload search, selected-node inspector, keyboard `/` focus, map pan/zoom/Fit; pan/zoom left stored coordinates unchanged. After review, keyboard Enter selection synchronises the inspector and ArrowRight persists +20 through Rust and reload. Group selection/movement is deliberately disabled to match the single-server inspector. Model test distinguishes host/kind/scope/engine identities and finds the last of 540 services.
- **T006 truthfulness:** all cards/inventory/activity use Rust MapState; reachability remained unknown. Saved/stale observations are labelled. Classified raw-marker exclusion checked. No simulated auth, metrics or server actions.
- **T007 desktop layout:** native renderer at 1280×720 and 1440×900, no document overflow, map/controls/footer visible; screenshots visually inspected. Fit gives an overview; zoom and workspace inventory provide readable detail without altering stored layout.
- **T007 inventory/state:** full saved service/container inventory searchable, source timestamps/scope/engine retained. Duplicate-host failure keeps form/data; corrupt native file shows startup error/retry without replacing it. Loading and empty states have explicit copy and no invented data.
- **T007 workspace:** Services/Files/Terminal/System/Settings/Transfers tabs are keyboard reachable. Services browse complete saved data; System displays static saved hardware. Protected/unimplemented controls show unavailable/no authenticated session. Native dialog Escape closes; inaccessible relationship select labels were corrected.

## Other checks and repairs

- `cargo test --workspace --locked`: **18 Rust tests passed** (16 existing core plus 2 native persistence/close/error tests).
- `node --test ui/client.test.ts ui/map-model.test.ts`: **4 tests passed**.
- `mise run check`: generated bindings, TypeScript, Rust fmt and Windows compilation passed. Known ts-rs attribute warnings remain as documented in F02; generated output matches.
- First UI run found ambiguous select labels and a dismissed error retained in a new dialog; explicit labels and error reset fixed them. Initial map fitting now waits for measured nodes.
- A proof harness attempt used DOM `window.close()`, which does not request Tauri native close in WebView2. Replaced it with the real native Quit action; native restart proof then passed. No product data was lost.
- Synthetic legacy digest unchanged; failures and test children were cleaned up by the harness. Private proof copies retained. No protected host access, infrastructure change or LAN listener.

## Review and limits

Independent bounded review found two P2 keyboard gaps: unsaved arrow movement and stale inspector selection. Both were fixed and independently confirmed resolved; final native regression above passed. No remaining correctness findings in the bounded slice.

Real-owner native read-only verification: `.tools/owner-desktop-proof-498139e70c744bcb9d7c8eeb0c7886c5/result.json` passed **4 servers / 540 saved workloads**, exact preserved IDs/names/hosts/positions, ready native storage and unchanged original digest. Only the native copy was opened; no owner-map test edits. This is technical verification, not owner acceptance.

Physical owner acceptance, installer/release, phone composition/touch/native adapters and remote coordinator/SSH/file/transfer functionality are not claimed. Android was not rebuilt for this desktop-only slice. Existing F01 APK remains a foundation artifact. Python and native stores are separate; changes in one do not sync to the other.
