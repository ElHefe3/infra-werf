# T021 acceptance — Windows interactive SSH terminal

Date: 2026-10-26<br>
Revision: `7c2e941` plus the uncommitted T021 working tree used for the native build and proof.<br>
Result: pass
Feature/ticket: F10 / T021

## Environment and target

- Windows x64 desktop, native Tauri/WebView2 app.
- System OpenSSH connected only to the disposable local Linux OpenSSH fixture created by `scripts/verify_native_terminal.mjs` through loopback Podman.
- Node 26.12.1, pnpm 12.15.0, Rust 1.99.1 MSVC, Tauri CLI 2.12.4.
- No owner server, production workload, sudo action, owner SSH configuration or trust entry was used or changed.

## Acceptance mapping

| T021 criterion | Evidence | Result |
| --- | --- | --- |
| Interactive keyboard input, UTF-8, control/interrupt, resize and desktop behavior | Native Windows proof: direct backend input/output included `café 世界`; xterm keyboard entry reached the remote login shell; the resize observer changed the remote PTY to 33 rows × 73 columns; Ctrl+C interrupted a foreground process and the same shell accepted another command. | Pass |
| Bounded output and cleanup on Lock/disconnect/expiry | `cargo test -p atlas-core --lib terminal --locked --offline`: 64 KiB output-buffer bound/overflow behavior passed. Native Lock cleared xterm scrollback, rejected stale writes and prevented queued keystrokes from appearing after fresh authentication. Session reaping uses the same `close`/`stop_terminal` cleanup path exercised by Lock; existing SFTP session tests cover idle and absolute expiry. | Pass; expiry was not time-accelerated in the native journey. |
| Fresh protected handles, same host identity, no replay or persistence | Startup uses a short fixed `true` authentication probe through a pinned `Peer`, then the terminal process retains that exact temporary host-key pin. The publication/Lock race test passed. Native proof verified stale input was not replayed, config and `known_hosts` hashes stayed unchanged, database/WAL/SHM contained none of the synthetic terminal markers, and the database digest remained unchanged. | Pass |

## Verification

- `cargo fmt --all` completed.
- `cargo test -p atlas-core --lib terminal --locked --offline`: 4 passed, 0 failed. This includes 64 KiB output bounds, Windows command quoting, authenticated `TerminalOpen` audit behavior, and the deterministic terminal-publication/Lock contention regression.
- Windows Tauri debug build completed with `--no-bundle`. It used the already-built Vite `dist` because pnpm 12's lockfile supply-chain verification stalled in this environment. The Vite production build had passed earlier in this ticket; no frontend source changed after that build.
- `scripts/verify_native_terminal.mjs`: pass. Results are retained in the sanitized [native result](../../.tools/native-terminal-proof-ff55fa9e-c9af-4cb9-bab4-626d56d337bb/result.json). The fixture identity file was removed after the run; only the final proof directory was retained.
- Independent read-only review: pass. The reviewer identified and the implementation closed a late-publication race between Lock and terminal startup.

## Repairs and limits

The native proof uncovered incorrect ConPTY attribute handle wiring and missing redirected-stdio handling; startup now passes the HPCON value directly and sets `STARTF_USESTDHANDLES`. Long-lived OpenSSH `-E` output did not provide a timely authentication signal, so startup now performs a fixed, bounded public-key probe and pins the exact session host key for the terminal connection. Terminal publication rechecks visibility while holding the slot mutex shared with Lock cleanup. Details: [D030](../discovery/D030-windows-conpty-terminal-startup.md).

The acceptance fixture is synthetic and loopback-only. No real LAN host compatibility or release installer is claimed here; T024–T026 cover integrated recovery, packaging and owner acceptance. Android and phone keyboard/lifecycle remain deferred in S09.

The temporary SSH fixture was removed on exit; generated failed-attempt proof directories containing one-time fixture keys were deleted. The final sanitized result JSON remains for traceability.
