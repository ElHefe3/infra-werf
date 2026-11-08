# T035 progress — forwarded web view for remote localhost services

Date: 2026-11-08. Feature: F17 / S11. Ticket: T035 (in progress: implementation proven, independent review pending; not an acceptance record).
Revision: working tree on top of `033eadf`; committed with this record. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`. Pins unchanged from [CHECKPOINT.md](CHECKPOINT.md); `windows-sys` gains the `Win32_System_JobObjects` feature (same locked version).

## What was built

- `atlas-core::tunnel`: one system-OpenSSH child per tunnel, `-N -L 127.0.0.1:<ephemeral>:localhost:<port>`, with every existing restriction (BatchMode, strict host keys, no agent/X11, no multiplexing, publickey only) plus `ExitOnForwardFailure=yes` and `GatewayPorts=no`. `ClearAllForwardings` is omitted only for this command; aliases whose configuration defines Local/Remote/DynamicForward are refused (`ssh -G` resolution without clearing). Children join a kill-on-close Windows job object, so they die with InfraWerf by any route. 4-hour absolute lifetime, 8 tunnels maximum, relative-path validation.
- Workspace: strict SSH verification (the Settings path) before every tunnel; lifecycle/endpoint recheck after start; reaping on child exit, expiry, archive/ignore/offline or endpoint change.
- Shell: `tunnel_open/close/close_all/list/focus` commands (main window only); each tunnel opens an InPrivate `app-<id>` window restricted to its loopback origin, with no capability grant. Closing it ends the tunnel; closing the main window ends all tunnels and exits. The close guard now applies to the main window only.
- UI: an **Apps** tab in the server workspace with port/path/name, per-server shortcuts (name/port/path in local storage only), open-tunnel list with Show window/Close and Close all.

## Checks

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 83 passed, 2 ignored (real-SSH fixture tests) |
| `mise run test:feature -- T035` | pass: 4 Rust tunnel tests, native check, contracts, tsc, 3 UI tests |
| UI tests (`client`, `map-model`, `live-model`, `remote-apps-model`) | pass: 14/14 |
| `node scripts/verify_native_apps.mjs` (`mise run test:apps-native`) | pass, 6 stages |
| Regression: `verify_native_monitoring.mjs`, `verify_desktop.mjs` on the same binary | pass |

The Rust test `configured_forwards_and_bad_inputs_are_refused` initially failed: resolution used `ClearAllForwardings=yes`, which hid configured forwards from `ssh -G`. Fixed before any native run. The first native exit test found that open application windows kept the process alive after the main window closed; the main window's destruction now ends all tunnels and exits.

## Acceptance mapping (native journey, loopback asyncssh fixture + private loopback HTTP/WebSocket app)

| Criterion | Evidence |
| --- | --- |
| Loopback-only, selected server only; clear errors without fallback | Forward URL is `http://127.0.0.1:<ephemeral>/`; the same port is not reachable on the PC's LAN address (192.168.20.50). Fixture saw only `localhost:<app port>` direct-tcpip requests and zero remote/other forwards. An alias with a configured LocalForward and invalid ports/paths are refused (`denied` / `invalid_input`). |
| HTTP and WebSocket work; window cannot invoke InfraWerf | Dashboard showed API `online` and WebSocket `pong`; `invoke('map_state')` from the app window was denied; navigation to another origin was blocked. |
| Close, Lock-equivalent, expiry, lifecycle change and exit terminate the tunnel | Real WM_CLOSE on the window → tunnel list empty, listener gone, SSH connection closed. Archiving the server → tunnel reaped and new tunnels refused. Quit with two tunnels → both listeners gone and SSH connections closed. Expiry uses the same reaping path (4 h; not waited out). |
| Trust/auth outcomes unchanged; nothing alters OpenSSH trust | Tunnels start only after `SshCheckState::Verified` from the existing verification; unknown/changed keys map to `HostTrustFailure`. No known_hosts or config writes. |

Artifacts (ignored, untracked): `.tools/apps-proof-*/` (`result.json`, `apps-tab.png`, `app-window.png`).

## Limits and open items

- Independent review of webview isolation and forwarding options is still required before closing T035.
- "Lock" in this app is per protected Files session; tunnels are not tied to it. They end on window close, Close all, server lifecycle change, expiry and exit.
- Sites that redirect to another origin (for example external single sign-on) are blocked inside the window by design.
- Not yet installed as the owner's primary release; the owner's installed copy is still the pre-F17 build (see D050).
