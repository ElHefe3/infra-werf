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

## Update 2026-11-08 — application discovery (owner request)

Owner: "add discovery so I can use a dropdown to open localhost/applications sites without needing to know their exact port or address."

- New read-only inspection `ApplicationDiscovery`: a fixed script over a verified, host-pinned session (the inventory-refresh path) lists listening TCP sockets with `ss -Hltnp` and reads each port's HTTP status and `<title>` (falling back to the first `<h1>`) with a 2 s, 64 KiB GET from the server itself. Only port, bind address, status, HTTPS-only flag, title and visible process name are returned and strictly parsed.
- The Apps tab runs discovery on open and offers a **Detected applications** dropdown (web apps openable; non-HTTP, HTTPS-only and HTTP 400 ports listed but disabled). Manual entry and shortcuts moved under "Enter a port manually".
- Apps bound to a specific server address (not `127.0.0.1`/`::1`/any) can be opened through that address only when the latest discovery for that server and endpoint reported that address and port; other addresses, ports and hostnames are refused.
- Checks: `mise run check`; 85 Rust tests (6 tunnel tests including strict discovery parsing and destination validation); 18 UI tests; native journey 7/7 on the 0.1.5 release binary (adds dropdown discovery and the specific-address refusals). The real discovery script was also run read-only on lab-server-2 and lab-server-3; it found the expected services (several web applications). This exposed and fixed IPv6 bracket handling, an empty status code and missing-title fallback.
- Installed as 0.1.5 to `%LOCALAPPDATA%\InfraWerf` after a data backup (`pre-0.1.5-install-20261108-130710`, `atlas.sqlite3` SHA-256 `C61323DFA3EEAC665E1D9A1A3516C44DBB99173B24FFFB4A5DB13B6972A87B54`). Installer SHA-256 `91C7ECB104D3DF250C577A8CEC45CC460C38FEF1542F2F2BB29153E31EFF3FE0`; installed executable SHA-256 `B4B66C376756E021DA155FDF811FFBC4E897407E8594D5E8DAC720851D005328`.
- Known limits: HTTPS-only apps cannot open in the window yet; apps that route by host name (such as a web application on `app.lan`) show their default site through the tunnel.

**Correction 2026-11-18:** the 0.1.5 "install to `%LOCALAPPDATA%\InfraWerf`" above was redirected into the Claude desktop package cache, not the owner's real folder. The owner installed 0.1.6 from Explorer on 2026-11-18 (verified ProductVersion 0.1.6 via the admin share). See [D063](../discovery/D063-claude-desktop-shell-is-also-packaged.md).
