# T013 desktop elevation progress

Date: 2026-10-26<br>
Result: **passed on Windows; controlled sudo/SFTP checks, native UI journey, workspace check and independent review pass**

ID: T013
Revision: 7c2e941 + T013 working tree
Result: pass

## Change

Windows OpenSSH's `-E` logfile cannot be read live while the process owns its exclusive writer. A plain stderr-pipe workaround also let a server's raw SSH authentication banner spoof client DEBUG text. The implementation sets `LogLevel=QUIET` and limits `LogVerbose` to `sshconnect.c:verify_host_key()`, `sshconnect.c:check_host_key()`, and `sshconnect2.c:ssh_userauth2()`. Parsers require the matching local source path, function, line, and PID prefix. Strict host checking uses the session's pinned temporary known-host entry. Atlas releases the random remote command gate only after the source-qualified fingerprint matches and the client reports public-key authentication.

For password-bearing operations, the fixed remote command redirects its own stderr to stdout. That keeps local OpenSSH diagnostics and the remote sudo prompt on distinct pipes. Atlas sends the password only after the authenticated session emits the random prompt token, removes that token from returned text and keeps all output within the existing broker limit. The rejected patch that removed the gate was not applied.

## Verification

- `cargo test -p atlas-core --lib ssh::tests::diagnostics_expose_only_valid_fingerprints_and_fixed_states --locked`: passed; tests source-qualified Windows log records and rejects forged pre-identification lines.
- `cargo test -p atlas-core --test elevation --locked -- --ignored --nocapture`: passed against a disposable loopback Linux fixture in 26.35 seconds. The proxy injects fake host-key and public-key-success text before the SSH identification string. The test opens a key-authenticated SFTP session, removes the disposable account's key, enables SSH none-auth, and verifies both connection check and sudo flow reject the spoof without delivering the password sentinel.
- The same fixture covers wrong-password rejection despite an existing global sudo timestamp, fresh one-action success, preservation of that timestamp, NOPASSWD denial, invalid input, cancel/replay, mismatched host/config, key rotation, expiry, child/lease cleanup, and redacted audit.
- `cargo test -p atlas-core --test files --locked -- --ignored --nocapture`: passed in 41.46 seconds; includes real SFTP browse/handle/revocation and transfer/file-mutation proof using the shared diagnostics.
- `cargo check --workspace --locked`: passed. Existing ts-rs serde-attribute parse warnings remain.
- Actual native elevation UI journey: passed on the rebuilt Windows Tauri app using the isolated Linux SSH/sudo fixture, a disposable database and a private WebView data folder. It verified authenticated SFTP session opening, no pre-review password field, cancel, wrong sudo password, fresh valid sudo, another independent challenge, then Lock. The lifecycle lease remained at one while the protected session was open and returned to zero after Lock. The map database, SSH config and known-hosts file were unchanged; fixture passwords were absent from UI text and persistent database. No real host was contacted. The visible native app was separately inspected on the Windows desktop after rebuild.
- Bounded independent re-review: **pass**, no concrete bypass found. Review confirmed exact function filters exclude `ssh_exchange_identification` and `input_userauth_banner`, and the proxy injects lines before the real SSH stream.

## Acceptance state

T013 acceptance is complete for the controlled desktop fixture. This proves the Windows native flow against an isolated Linux host; it does not claim privileged changes on a real server. Owner-facing release acceptance remains T026.

The original logfile-sharing reproduction and rejected-gate-removal history remain in [T013 blocker evidence](T013-blocker.md) and [D023](../discovery/D023-windows-ssh-log-sharing.md). No real host, SSH trust file, password, key or sudo policy was used or changed.
