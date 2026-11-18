# T047 — Independent review of plugin view isolation

Date: 2026-11-17. Reviewer: a separate session, as decided in [D059](../discovery/D059-plugin-views-tab-confirm-and-review.md). Reviewed commit `d62feac` (Isolated plugin views with a message API) on a branch cut from `feature/s12-extensibility`. No product code was changed. Proofs of concept are in [`scripts/fixtures/hostile-view-review/`](../../scripts/fixtures/hostile-view-review/run-review.mjs), and the evidence from the last live run is in [`T047-isolation-review/`](T047-isolation-review/review-result.json).

## Verdict

The core boundary holds. A plugin page could not reach Tauri IPC, InfraWerf's commands or events, the main window's DOM, the filesystem outside its folder, or any HTTP(S)/WebSocket origin other than its own. Lock, closing the view and the kill switch tore it down and closed its port.

The claim "cannot reach any network origin other than its own" is **false**. WebRTC is not governed by the content security policy. A page sent UDP packets carrying data it chose to an address it chose. The data was the server's name and host, which the page reads with no grant at all (finding 1). Three medium findings also weaken the owner's control and the audit trail:

- A page can wipe the whole plugin activity log in about ten seconds (finding 2).
- The Run button appears under the view's area about 60 ms after the page asks (finding 3).
- Any local process can tie up one thread per connection on a view's port (finding 4).

I recommend fixing 1–3 before T047 closes. 4 and the low findings can follow.

| # | Severity | Finding |
| --- | --- | --- |
| 1 | **High** | WebRTC sends page-chosen data to any address, bypassing the CSP |
| 2 | Medium | The activity log can be flooded without limit; every other entry is pruned |
| 3 | Medium | The Run button appears under the view and is enabled at once (click-through timing) |
| 4 | Medium | The view server starts one thread per connection without limit; slowloris holds them |
| 5 | Low | The 40-entry block cap can be spent first by the page or by any local process, after which nothing more is logged |
| 6 | Low | A page can write text of its own choosing into the activity log through `csp-report` and `blocked` |
| 7 | Low | An NTFS short name gets past the dot-name check (`HIDDEN~1.JSO` serves `.hidden-config.json`) |
| 8 | Low | View files are served as they are on disk now, not as reviewed; intermediate links are not checked |
| 9 | Low | Answering an action has no "running" state; a newer request replaces the dialog's request |
| 10 | Info | A `blob:` navigation is allowed, and the guard script does not take effect there; isolation still held |
| 11 | Info | Defence in depth if the guard and CSP fail: Tauri's ACL; one ACL exception (`__TAURI_CHANNEL__|fetch`) is harmless today |
| 12 | Info | `apps.rs` emits to `"main"` (any label), not to the main webview |
| 13 | Info | Not tested: DNS prefetch, `window.print`, fullscreen, notifications, clipboard via `execCommand` |

## How it was tested

- **Source reading.** I read `plugin_view.rs`, `workspace/views.rs`, `lib.rs`, `apps.rs`, `capabilities/default.json`, `build.rs`, `ui/plugin-view.tsx`, `ui/dialog.tsx`, the hostile fixture and the suite. I also read Tauri 2.12.5 (`webview/mod.rs` `is_local_url` and `on_message`, `ipc/authority.rs` `resolve_access`, `event/listener.rs` `emit_js_filter`).
- **Command gate.** A script listed every `#[tauri::command]` in `lib.rs` (75) and `apps.rs` (7) and checked that the first statement of each calls `local_desktop`. All 82 do.
- **Live runs.** The live runs used the release build of `d62feac` (`pnpm tauri build --no-bundle -- --locked`, built 17:09). Run it with:

  ```bash
  ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe mise run exec -- node scripts/fixtures/hostile-view-review/run-review.mjs
  ```

  The runner starts InfraWerf with a disposable database, an empty SSH config, a plugins folder in `%TEMP%` (on C:, where NTFS short names exist) and a WebView2 debugging port. It also starts a TCP listener and two UDP listeners that stand in for "another origin", on 127.0.0.1 and the LAN address. It then adds a server named `View host` with host `secret-host.home.arpa` and enables the fixture plugin `org.example.hostile-view-review` with `read_saved_state`, `ssh_session` and `run_approved_command`. Finally it opens the `Review attempts` tab and drives four phases by writing `go-N.txt` into the plugin folder. Results were judged from the listeners, InfraWerf's own state and its SQLite database, not from the page's own report.
- Runs 6 and 7 gave the same results. Run 6 accidentally imported servers from the real `~/.ssh/config` into its disposable database (`ATLAS_SSH_CONFIG` was unset). Nothing was written outside the temporary folder. The runner now sets an empty SSH config, and run 7 is the evidence quoted below.
- No `atlas-native.exe` was running before each run.

## Findings

### 1. High — WebRTC sends page-chosen data to any address, bypassing the CSP

**What.** Chromium does not apply the content security policy to `RTCPeerConnection`. The CSP has no `webrtc` directive that Chromium enforces, and the Permissions-Policy header does not cover WebRTC. A view can therefore send UDP to any host and port:

- through STUN/TURN servers it names in `iceServers`, or
- through ICE connectivity checks to a remote candidate it writes into a hand-made SDP answer. The STUN `USERNAME` attribute of every check is `<remote ufrag>:<local ufrag>`, and the page chooses the remote ufrag, up to 256 characters.

This is a working exfiltration channel to the internet. The `context` call alone, which needs no grant, gives the server's name and host. With `read_saved_state`, the page can send the inventory and observations too.

**Reproduce.** See `rtc_stun_*` and `rtc_checks_*` in `plugin/view/attack.js`, and the UDP listeners in `run-review.mjs`.

**Evidence** (run 7, `T047-isolation-review/review-result.json`):

- The UDP listener at `127.0.0.1:<udp2>` received **96 STUN binding requests** from WebView2. Their USERNAME was `X5669657720686f73747c7365637265742d686f73742e686f6d652e617270:UGe0`.
- Hex-decoded, that is `View host|secret-host.home.arpa`, exactly what `GET context` returned.
- The STUN listener received 8 binding requests, from 127.0.0.1 and from the LAN interface.
- The page's TCP attempts (preconnect, prefetch, speculation-rules prerender, preload, CSS `@import`/`url()`, SVG `<image>`, `sendBeacon`, `EventSource`, `<a ping>`, WebTransport) produced **no** connection at the TCP listener.

**Suggested fix.** Turn WebRTC off for views:

- Start WebView2 with `--force-webrtc-ip-handling-policy=disable_non_proxied_udp`. The main webview does not use WebRTC. The child webviews share the environment, so set it where the main window's environment is created; with no proxy configured, this disables WebRTC UDP.
- Also remove `RTCPeerConnection`, `webkitRTCPeerConnection`, `RTCDataChannel` and `RTCIceTransport` in the guard script, as a second layer. The guard is not reliable on its own: see finding 10.
- Add `rtc_checks` to the native suite, asserting that the UDP listener receives nothing.
- Change `views.md` so it no longer promises more than is enforced until this is done.

### 2. Medium — The activity log can be flooded without limit; every other entry is pruned

**What.** Only `blocked()` is capped at 40 rows. Two other paths write a row on every call:

- `ViewLink::ask` writes a `view_request`/`refused` row, with the page's `contribution_id` as detail, on every refused request.
- `observations` and `inventory` call `plugin_authorize`, which also writes a row on every call.

`plugin_activity` keeps only the newest `history_limit` rows (1000 by default) across **all** plugins. A view can therefore erase the log:

- its own `enable` and `set_permissions` rows,
- all of its `view_blocked` rows, which are the evidence of its escape attempts,
- every other plugin's activity.

**Reproduce.** Phase 2 of `attack.js` sends 1050 `POST actions` with `contribution_id: "flood-<i>-AAAA…"`.

**Evidence.**

- The flood took 9.6 s for 1050 requests.
- Afterwards the database held 1000 rows, ids 95–1094: 999 `view_request/refused` rows and the 1 later `needs_confirmation` row.
- Rows 1–94 were deleted. They included `enable`, `set_permissions`, `open_view` and all 40 `view_blocked` rows.
- The detail of each flood row is the page's own text, cut to 160 characters.

**Suggested fix.**

- Give each opening one budget for **every** row a view causes: blocks, refusals and capability checks. When the budget runs out, write a single "N further calls not logged" row when the view closes.
- Log a refused `contribution_id` only if it names a real contribution of the plugin; otherwise use a fixed word.
- Consider not logging allowed reads on every call, or coalescing them, as the page can poll.
- Prune per plugin, or keep security-relevant rows (`enable`, grants, `view_blocked`) apart from routine ones.

### 3. Medium — The Run button appears under the view and is enabled at once

**What.** The view covers the tab area. When it posts `actions`, InfraWerf hides it and shows its dialog, and the dialog's Run button sits inside the rectangle the view occupied. The page knows the moment it asks, so it can ask for a double-click ("click twice to load"): the first click sends the request, and the second lands on Run.

D059 says "A click inside plugin content never counts as the owner's". This does not break that literally, but a click *aimed at* plugin content counts as Run. The command is limited to the plugin's reviewed pack commands on that server (D057), which bounds the impact.

**Evidence** (run 7, `T047-isolation-review/dialog-over-view.png`):

- The dialog was visible **≤61 ms** after the page sent the request. Run 6 measured ≤77 ms.
- The Run button was at x 809–855, y 494–527.
- The view's area was x 179–1101, y 274–726, so Run was inside it.
- Run was enabled immediately.

**Suggested fix.**

- Keep Run disabled for about 1 s after the dialog appears, and again whenever the window regains focus. A visible countdown makes this clear.
- Ignore a click unless its pointer-down also happened inside the dialog.
- Optionally make Decline the default focus, so a stray Enter declines.

### 4. Medium — One thread per connection without limit; slowloris holds them

**What.** `ViewServer::start` spawns a thread for every accepted connection and imposes no limit. `IO_TIMEOUT` applies to each read, so a client that sends a header line every few seconds keeps its thread alive until 16 KiB of headers have arrived, which can take hours. Anything on the PC can connect to the loopback port, including other users' processes, and needs no secret, because the headers are read before the secret is checked.

**Evidence.**

- InfraWerf had **23 threads** before 1000 idle connections to the view's port and **1023** after.
- It still had 1023 after 17 s of trickling one header line every 7 s.
- The view still answered (`context` returned 200), so this is resource exhaustion, not an immediate outage.

**Suggested fix.**

- Cap concurrent connections per view (16 is plenty for one page) with a small fixed pool.
- Set an overall deadline per request (for example 5 s from accept to the end of the headers).
- Optionally refuse connections whose first bytes are not a request line within 1 s.

### 5. Low — The 40-entry block cap can be spent first; afterwards nothing is logged

**What.** The page can spend the cap on harmless junk calls, 45 `GET junk-N` in phase 2, and then nothing it does is logged. Any local process can also spend it by sending complete requests without the secret: `no_secret` and `wrong_host` count against the view's cap. Nothing in the UI shows the suppressed count.

**Evidence.**

- After phase 2, the page's `blob:` document tried an inline script and a cross-origin fetch. Both were blocked, and neither was logged: `blocked_after_blob: []`.
- A `no_secret` entry from the runner's own connections appears in the view's log.

**Suggested fix.**

- Write a final "N more blocked attempts" row on close.
- Keep separate small budgets per kind of block.
- Do not charge requests from outside the view (`no_secret`, `wrong_host`) to the plugin. Count them separately, without the plugin's name.

### 6. Low — Page-written text in the activity log

**What.** `csp-report` takes `effective-directive` straight from the page's JSON body. `POST blocked` takes `what` from the page. Both are written to the log as if InfraWerf had observed them.

**Evidence.** The log row reads `content_policy: InfraWerf: all good, nothing blocked https://trusted.example`.

**Suggested fix.**

- Map the directive to a fixed list (`connect-src`, `img-src`, and so on, or `other`).
- Accept only a known list of `what` words from the guard.
- Treat these rows as "the page reported", not as InfraWerf's own observation.

### 7. Low — An NTFS short name gets past the dot-name check

**What.** `handle` refuses path parts that start with `.`. On volumes with 8.3 names, which is the default on the system drive where `%LOCALAPPDATA%` lives, a dot-file also has a short name such as `HIDDEN~1.JSO`, and Windows resolves that name when opening the file.

**Evidence.**

- `GET HIDDEN~1.JSO` and `GET hidden~1.jso` returned the contents of `view/.hidden-config.json`.
- `GET .hidden-config.json` returned 404.
- The impact is small, because the files are the plugin's own, but the documented rule does not hold.

**Suggested fix.** Serve only names that appear exactly in the plugin's reviewed file list (see 8). Or refuse any part containing `~`, and compare the opened file's real name, read from the directory, with the requested one.

### 8. Low — Files are served as they are on disk now, not as reviewed

**What.**

- The folder hash is checked only at start-up (`plugins_verify`). The asset server reads files from disk on every request.
- `symlink_metadata` checks only the final path component, so a junction in an intermediate folder would be followed. I did not test that live.

Whatever can write the plugins folder already runs as the owner, so this is not an escalation. It does mean "unchanged since review" is not what the view shows.

**Evidence.** The runner rewrote `view/config.js` and added `go-2/3/4.txt` after InfraWerf had started. All were served.

**Suggested fix.** When a view opens, read the folder with `read_folder` (links refused), check it against the recorded hash, and serve from that in-memory set.

### 9. Low — No "running" state; a newer request replaces the shown one

**What.** `answer_view_action` releases the lock while the action is still `Waiting`, then runs the command. Two concurrent answers for the same ID would both run it. Only the main webview can call this, and the dialog disables its buttons while busy, so it is not reachable from a view today.

`PluginActionDialog` also replaces `request` on every `atlas-plugin-action` event. If a second view asks while the owner is reading the first request, the dialog changes under the cursor, and the first request stays `waiting` (and that view `busy`) until it closes.

The kill switch does not stop a run already started. That is by design: `run_pack_command` re-checks grants and marks an epoch change as cancelled.

**Suggested fix.**

- Set a `Running` state under the lock before running.
- Queue requests in the dialog instead of replacing them, and decline or cancel the replaced one.

### 10. Info — `blob:` navigation is allowed and the guard does not take effect there

**What.** `on_navigation` compares `target.origin()`, and for `blob:http://127.0.0.1:<port>/…` the `url` crate returns the inner origin. A view can therefore navigate itself to a blob document. In that document:

- The CSP was inherited: an inline script did not run, and a cross-origin fetch was blocked.
- `chrome.webview` was absent.
- `__TAURI_INTERNALS__` was present, and `invoke("map_state")` never answered.
- `window.ipc` **was present**, so the guard's trap was not in force.

Isolation held because the IPC route was absent and Tauri's ACL would refuse anyway (see 11). The guard is not a reliable layer.

**Suggested fix.** Refuse `blob:`, `data:` and `about:` navigations in `on_navigation` explicitly, requiring scheme `http` and the exact host and port. Do not rely on the guard for anything, including finding 1.

### 11. Info — Defence in depth if the guard and CSP fail

**What.** I checked the author's reading of Tauri 2.12.5:

- **`is_local_url`.** It treats as local the Tauri protocol, URLs relative to the app URL (`http://tauri.localhost/`; `devUrl` is `http://localhost:1420`), and registered custom protocols. `http://127.0.0.1:<random port>` matches none of these, so it is remote.
- **`on_message`.** It requires a resolved ACL whenever the origin is not local (`!is_local`).
- **`resolve_access`.** It matches a capability by webview label **or** window label, and also by origin context. `foundation` has `windows: []` and `webviews: ["main"]`, with no remote URLs, so a `pluginview-N` webview in window `main` gets nothing. Plugin commands such as `dialog` are always ACL-checked.
- **Commands.** All 82 commands call `local_desktop` first, and it requires webview `main` in window `main` showing the bundled origin.

So the guard script's absence would not matter for commands. The one exception is `plugin:__TAURI_CHANNEL__|fetch`, which Tauri lets through without an ACL. InfraWerf creates no `Channel`s and its main webview uses the `ipc.localhost` protocol, so there is no channel data to steal today. Keep it in mind if Channels are ever added.

### 12. Info — `apps.rs` emits to `"main"`, not to the main webview

`apps.rs` uses `app.emit_to("main", CHANGED, ())`, which is `EventTarget::AnyLabel` and contradicts D059's "events go to the main webview only". Tauri evaluates events only into webviews with registered JS listeners (`emit_js_filter`, keyed by webview label). A view cannot register listeners and the payload is empty, so nothing leaks. For consistency, use `main_webview()`.

### 13. Info — Not tested

- DNS prefetch (`<link rel=dns-prefetch>`), which can't be observed without a DNS server in the harness. Chromium does not apply CSP to it, so it may leak data in hostnames; check it with a local resolver.
- `window.print()`, element fullscreen, `Notification.requestPermission()`, and clipboard writes via `document.execCommand('copy')` on a user gesture.
- An intermediate junction in a plugin folder (see 8).
- Request smuggling and oversize requests were reviewed in source only:
  - one request per connection, and `Connection: close`;
  - `Transfer-Encoding` refused;
  - headers capped at 16 KiB and bodies at 64 KiB;
  - a duplicate `Content-Length` takes the last value, which is harmless with one request per connection.

## Claims confirmed, and how

| Claim | Result | How |
| --- | --- | --- |
| No Tauri IPC from the view | Confirmed | `invoke` from the page and from a `blob:` document never answered. `chrome.webview` was absent. Tauri source shows that a remote origin needs a capability, and none names the view (11). The author's suite covers `ipc.localhost` and `postMessage`. |
| No InfraWerf commands | Confirmed | All 82 commands call `local_desktop` first. The map was unchanged after every run (`map_after: ["secret-host.home.arpa"]`). |
| No InfraWerf events | Confirmed | Events are evaluated only into webviews with registered listeners, which a view cannot register (12). |
| No main-window DOM | Confirmed | It is a different origin and renderer. Fetch and frames to `tauri.localhost` were blocked (author's suite). The view's `localhost:<port>` fetch was blocked. |
| No filesystem outside its folder | Confirmed, with 7 and 8 | `..` and encoded forms are refused (`clean_path`, unit test). Short names reach only the plugin's own files. Junctions were not tested. |
| No network origin but its own | **Refuted** | WebRTC (1). All the HTTP(S), WebSocket, WebTransport, worker, service worker, beacon, ping, hint and prerender attempts were blocked. Downloads and popups were refused. |
| Reads only what grants allow | Confirmed | Each call re-checks `plugin_authorize`. `context` needs no grant, by design, but it gives server name and host, which finding 1 can send out. |
| Actions only after Run in InfraWerf's dialog | Confirmed, with 3 | Nothing ran without Run. The dialog is in the main webview and the view is hidden while it shows. The request is bound to the view's plugin, server and label (`action` filters by label; `answer` uses the stored server). |
| Torn down on Lock, close and kill switch | Confirmed | After Lock, `plugin_views_open` was `[]` and the port was **closed** (run 7). The author's suite covers close and the kill switch, and `close_plugin_view` cancels waiting requests. `ViewLink::call` returns `closed` once the view is gone. |
| Secret path | Confirmed | A 122-bit random UUIDv4, new on every opening. `Referrer-Policy: no-referrer`. It is not logged (the log keeps scheme and host only). The incognito profile keeps no history. |
| Host check against DNS rebinding | Confirmed | The `Host` header must equal `127.0.0.1:<port>` exactly (unit test, and `localhost:<port>` was refused). |

## Re-test of 943d915

Date: 2026-11-17. I merged `943d915` (Fix the plugin view isolation review findings) into this branch (`c1c8ae6`), and rebuilt with `pnpm tauri build --no-bundle -- --locked` (release, 17:57). The re-test used the same runner, with these changes:

- The phases are now signalled over the debugging connection (`window.GO`), because the view no longer serves files added after it opens.
- The activity database is read after InfraWerf quits.
- New phase-1 attempts try WebRTC from every other realm I could reach.

Runs 8 and 9 gave the same results. Run 9 is the evidence: [`retest-943d915-result.json`](T047-isolation-review/retest-943d915-result.json) and [`retest-943d915-dialog.png`](T047-isolation-review/retest-943d915-dialog.png). No `atlas-native.exe` was running before each run.

### Verdict

Findings 1–10 and 12 are **fixed** as far as I could test. One new low finding (14) and two observations (15, 16) remain. Nothing new is high or medium. Finding 11 (`__TAURI_CHANNEL__|fetch`) and 13 (untested items) were not addressed, as stated.

| # | Was | Now | Evidence |
| --- | --- | --- | --- |
| 1 | High | **Fixed** | 0 UDP and 0 TCP packets reached the listeners. 21 WebRTC attempts failed: 4 direct and 17 from other realms. |
| 2 | Medium | **Fixed** | After 1050 refused requests the log held 45 rows, and nothing was pruned. |
| 3 | Medium | **Fixed** | Run stayed disabled for about 1.36 s after the dialog appeared, and Decline had focus. |
| 4 | Medium | **Fixed**, with new low 14 | 1000 idle connections: 22 → 38 → 22 threads. |
| 5 | Low | **Fixed** | One budget, a summary row on close, and outside requests are not charged. |
| 6 | Low | **Fixed** | A forged directive is logged as `other`. |
| 7 | Low | **Fixed** | `HIDDEN~1.JSO` and `hidden~1.jso` returned 404. |
| 8 | Low | **Fixed** (by design: in-memory set at open) | Files are not re-read; the runner's signal had to move off files. |
| 9 | Low | **Fixed in code** (not reachable from a view, not tested live) | `Running` is set under the lock; requests queue in the dialog. |
| 10 | Info | **Fixed** | Navigation to a `blob:` document was refused, and no blob page appeared. |
| 11 | Info | Unchanged, as stated | — |
| 12 | Info | **Fixed in code** | `apps.rs` emits to the main webview. |
| 13 | Info | Unchanged, as stated | — |
| 14 | — | **New, low** | A local process holding the 16 slots makes all of the page's own requests fail. |
| 15 | — | **New, info** | Once the budget is spent, the view's real action request is not logged on its own. |
| 16 | — | **New, info** | Connections closed at the cap are not counted as outside requests. |

### Per finding

**1 — WebRTC: fixed.** With the guard in place, every route I tried failed and the UDP listeners received nothing (`hits: {tcp: 0, udp: 0, udp2: 0}`).

- **Direct STUN and ICE checks.** All four attempts (loopback and LAN, `iceServers` and a hand-made remote candidate) failed with "PC is not a constructor" or "no constructor".
- **Other realms**, each tried from the top page with `checks()` and a username tagged with the realm's name. In all 17, `RTCPeerConnection` was `undefined`:
  - a synchronously created blank frame;
  - `window[window.length-1]`;
  - an explicit `about:blank` frame;
  - an `srcdoc` frame, both after load and read synchronously;
  - an `srcdoc` frame sandboxed with `allow-scripts allow-same-origin`;
  - a blank frame nested in a blank frame;
  - a blank frame after `document.open()`/`write()`/`close()`, read at once and again 200 ms later through a window reference taken before;
  - a detached frame's window;
  - a frame moved to `<head>`;
  - a frame after `location.reload()`;
  - a frame whose `srcdoc` was changed after load;
  - an `<object data="about:blank">`;
  - the property descriptor on `window`;
  - routes through the prototypes of `RTCRtpTransceiver` and `RTCSessionDescription`, which remain as globals.
- **Function constructors.** Reaching one through a getter's `.constructor` was refused by the CSP (no `unsafe-eval`). So were `eval` and `Function` from frames, since about:blank and srcdoc frames inherit the policy.
- **Other contexts.** Workers and service workers are refused by `worker-src 'none'`. An AudioWorklet loads from `'self'` but has no WebRTC. Popups are refused, and a `blob:` document can no longer be reached.
- **The browser flags.** The author found that `--force-webrtc-ip-handling-policy` still let STUN out, so WebRTC is now held only by the guard script, which must run in every realm. It did in every realm I could create. That rests on WebView2 running `initialization_script_for_all_frames` before any page script in every new document. Keep the frame-realm attempts in the native suite so a WebView2 update that changes this is caught.
- **Not re-tested live.** `context` now returns `server: null` without `read_saved_state`. This fixture holds the grant, so it still received the server; the author's suite covers the refusal.

**2, 5, 6 — log flooding, cap and page text: fixed.**

- 1050 refused `actions` requests took 4.6 s. Afterwards the database held **45 rows**: `enable`, `set_permissions`, `open_view`, and 42 `view_blocked` rows. That is 40 within the budget plus 2 summary rows written on Lock:
  - "1096 more refused or blocked attempts were not logged one by one"
  - "1 requests from outside the view were refused (not counted as the plugin's)"
- Nothing older was pruned.
- The forged CSP report was logged as `content_policy: other https://trusted.example`.
- A flood row now carries `unknown_contribution`, not the page's text. The flood rows were not logged one by one in this run, but this was confirmed in the code.

**3 — Run timing: fixed.**

- The dialog was visible ≤88 ms after the request, and Run is still placed inside the view's former area.
- The button reads "Run (wait…)" and is disabled. It first became the enabled "Run" button **about 1.36 s** after the dialog was seen.
- Decline had focus.
- A click counts only when its pointer press started on the button, and window focus restarts the wait.

A click aimed at the page within the first second can no longer land on Run.

**4 — threads: fixed; see new finding 14.** With 1000 idle connections the thread count went from 22 to 38 (16 handlers plus the listener). It returned to 22 after the 5 s header deadline, even while a header line was trickled every 7 s. The page's own request still answered 200 once the idle connections were gone.

**7, 8 — short names and live files: fixed.**

- `HIDDEN~1.JSO`, `hidden~1.jso` and `.hidden-config.json` all returned 404.
- The view serves the set `read_folder` read when it opened, links refused, and checked against the install hash. My runner's old signal, files written into the folder after the view opened, stopped working, which confirms that nothing is re-read from disk.

**9, 12: fixed in code** (read in the diff; neither is reachable from a view):

- `answer_view_action` sets `Running` under the lock.
- The dialog queues requests and says how many are waiting.
- `apps.rs` now emits to `main_webview()`.

**10 — `blob:`: fixed.** Navigating to `blob:http://127.0.0.1:<port>/…` was refused. No blob page appeared within 20 s, and the view stayed on its page.

### New findings

**14. Low — A local process can deny a view its own server.** Since 943d915 the server refuses connections beyond 16 at once.

- **What happens.** Another program on the PC needs no secret: it can keep 16 connections open, each sending a partial request line and reopened as soon as the 5 s deadline drops it. Then the page's own requests fail.
- **Evidence.** Holding the slots this way, all 8 of the page's `context` requests, one a second, failed with "Failed to fetch". It answered 200 again once the program stopped.
- **Impact.** Only the view's availability, not its isolation. A program that can do this can already disrupt the owner in many other ways, so this is low.
- **Suggested fix.** Reserve a few slots for connections whose request carries the secret path, by reading the request line before counting the connection against the shared limit. Or give a shorter deadline (about 1 s) to connections that have not sent a complete request line.

**15. Info — Once the budget is spent, the view's real request is not logged by itself.** After the flood, the page's legitimate `POST actions` for `kernel` opened InfraWerf's dialog, but no `view_request` / `needs_confirmation` row was written, because `ask` logs it through the shared budget. The only trace is the summary count at close, and the owner's Decline left no row either. Requests that reach the owner's dialog are already limited to one waiting per view by the owner's own answers. Suggest logging them, and the owner's answer, outside the budget.

**16. Info — Connections closed at the cap are not counted.** The "requests from outside the view" row counted 1, although 1000 connections were made. Those closed at the cap, or dropped at the header deadline before sending a request, are not counted anywhere. It is fine not to charge them to the plugin, but the summary row understates the attempts. Suggest counting connections refused at the cap and those timed out at the deadline in the same row.
