# T047 acceptance — isolated plugin view with a message API

Date: 2026-11-17. Feature: F23 / S12. Ticket: T047.
Revision: bec7d39
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `bec7d39` records. Commits: `d62feac` (views), `943d915` (review fixes), `bec7d39` (re-test fixes).

Owner decision first: a workspace tab in a child webview, actions only after the owner presses Run in InfraWerf's own dialog, and an independent review of the isolation ([D059](../discovery/D059-plugin-views-tab-confirm-and-review.md), 2026-11-17).

## What was built

- **The view.** A `workspace_tab` contribution may carry `view.entry`, a page in the plugin. It appears as a tab where the plugin is enabled, intact and allowed, and opens as a Tauri child webview laid exactly over the tab area (checked to within 2 pixels), in a private profile with devtools off.
- **Its own origin.** The page is served by a loopback server at `http://127.0.0.1:<random port>/<secret>/`, a new 122-bit secret on every opening, which Tauri treats as remote, so it refuses IPC from it. Every response carries a strict content security policy: no other origin, frames, workers, forms or objects. `Host` must be exact. Navigation is allowed only to plain `http` on the same origin, so `blob:`, `data:` and `about:` are refused; new windows and downloads are refused.
- **What it serves.** The plugin's files are read once when the view opens, links refused, checked against the hash recorded at install, and served by exact name from memory.
- **Every command checks its caller.** All 82 commands take the calling `Webview` and admit only the webview `main` in window `main`; ten of them checked nothing before. The core capability names the `main` webview, not the window. Events go to the main webview only.
- **The guard.** A script runs in the page and every frame it creates before the page's own code. It removes the native message channel and WebRTC, and reports attempts to use them. WebRTC ignores the content security policy and WebView2 has no setting that stops it: two browser flags were tried and measured, and STUN packets still went out, so they are not used.
- **Message API v1.** `context`, `observations`, `inventory`, `actions` and `actions/<id>` under `/<secret>/infrawerf/v1/`. Each call is decided against the owner's grants at that moment. The server's name and host need `read_saved_state`.
- **Actions.** A request opens InfraWerf's own dialog showing the plugin, the server and the exact command. The view is hidden meanwhile. Run stays disabled for a second, and again whenever the window regains focus, and counts only a press that began on it. Decline has the focus, and requests queue. It runs through the same checks as the Plugin data tab and is marked running under a lock.
- **Teardown.** Lock view, closing the workspace or changing tabs, and any plugin permission change (disable, permissions, trust, kill switch) close the view and its server and cancel its waiting requests.
- **Activity log.** One budget of 40 rows per opening covers blocked attempts, refused requests and refused reads, with a summary row on close. Requests from outside the view (no secret, wrong host, other origin, refused at the cap, malformed or too slow) are counted in a separate row and not charged to the plugin. Page-written text is mapped to fixed words. Requests that reach the owner, and the owner's declines, are always logged.
- **Limits.** At most 32 connections that have not sent their request line (1 s deadline) and 16 requests carrying the secret, each with a 5 s header deadline.
- **Documents.** `docs/plugin-api/views.md` (the field, the guarantees, the message API v1, error codes and versioning); the example `examples/host-dashboard`.

## Checks on bec7d39

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 142 passed, 2 ignored (need a real SSH fixture) |
| UI tests | pass: 36 of 36 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on bec7d39

All 17 suites passed in one uninterrupted run of `ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe mise run test:desktop:full`: desktop, ssh, health, inventory, files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps, packs, pack-map, plugin-install, plugin-update, plugin-views.

The new suite `plugin-views` (`scripts/verify_native_plugin_views.mjs`) drives the built application with the hostile fixture view `scripts/fixtures/hostile-view` and the example view, and runs real commands in the disposable Linux container.

## Independent review

A separate session reviewed `d62feac` and tried to break it ([report](T047-isolation-review.md), commits `3570cca` and `8591c59`, merged from branch `claude/view-review-02e7db`).

- **The core boundary held.** No IPC, commands, events, main-window DOM or other HTTP origins were reachable.
- **One high finding.** WebRTC sent page-chosen data over UDP to any address.
- **Three medium findings.** The activity log could be flooded and pruned; the Run button appeared under the view and was enabled at once; the view server had no thread limit.
- **Lower findings.** Six low and four informational findings.

All but two of the informational findings were fixed in `943d915`. The reviewer then re-tested that commit and found findings 1–10 and 12 fixed:

- 21 WebRTC attempts from every realm it could reach found no `RTCPeerConnection`, and no packets arrived.
- After 1050 refused requests the log held 45 rows.
- Run waited about 1.36 s.
- Idle connections no longer accumulate threads.

The re-test raised one new low finding (14) and two informational ones (15, 16), fixed in `bec7d39`.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| A fixture view renders and reads only the observations its grants allow | Native suite: the example view showed the server name, its plugin's reading and the saved inventory, and its request to run `uname -r` showed the container's real kernel after Run. The hostile view, allowed only `ssh_session`, was refused observations, inventory, a command and the server's name and host (`not_granted`, logged). Core test `a_plugin_view_is_offered_only_where_the_owner_allowed_it`. |
| Attempts to reach IPC, other origins or ungranted data fail and are logged | The hostile view's attempts all failed, and the map was unchanged (no planted server). Its attempts covered: Tauri internals and `add_server`; `window.ipc` and `chrome.webview`; fetches to `ipc.localhost`, InfraWerf's page and the internet; image, WebSocket, frames, workers, `window.open` and forms; WebRTC from the page and from blank, indexed, srcdoc and nested frames (0 UDP packets; InfraWerf's own page as positive control sent 4); the API without the secret; and navigation away. The activity log holds content-policy, guard, unknown-call and new-window rows within the budget, plus summary rows. The independent review confirmed and extended this. |
| Closing, Lock and the kill switch tear the view down | Native suite: Lock view, closing the workspace and turning all plugins off each closed the webview within seconds (`plugin_views_open` empty, the page gone). A request waiting at the kill switch was cancelled and ran nothing when Run was pressed afterwards. The reviewer confirmed the view's port closed on Lock. |
| The message API is documented and versioned | `docs/plugin-api/views.md`: every call, field, error code and state; version `v1` in every path and in `context`, with the rule for adding and breaking changes. |

## Not done, and why

- **WebRTC is held by the guard script only.** WebView2 offers no switch that stops it; measured. The guard runs in every frame the page can create, in the author's and the reviewer's tests. A WebView2 update that changed when initialization scripts run could weaken this; the native suite's frame attempts would catch it.
- **A local program can still deny a view service** by holding the 32 connection slots that have not sent their request line, reopening them every second. It cannot reach data or the page's secret. Review finding 14, reduced but not removed. A program that can do this can already disrupt the owner's PC.
- **Not tested** (review finding 13): DNS prefetch, `window.print`, fullscreen, notifications, clipboard writes on a user gesture, and an intermediate junction in a plugin folder. The last is moot now: files are read with links refused when the view opens.
- **`plugin:__TAURI_CHANNEL__|fetch`** passes Tauri's ACL without a capability. InfraWerf creates no channels, so there is nothing to fetch today (review finding 11).
- The review's report, its fixture and runner (`scripts/fixtures/hostile-view-review/`) and its evidence were merged on the owner's instruction.

## Notes

- **Tauri's `unstable` feature is now on.** With it, the desktop crate's test binaries could not load (`STATUS_ENTRYPOINT_NOT_FOUND`). `build.rs` now embeds Tauri's own Common Controls manifest in every target through the linker, instead of only in the application.
- **A view's own page cannot use `read_saved_state` reads to fill the log:** allowed reads are not logged one by one.
