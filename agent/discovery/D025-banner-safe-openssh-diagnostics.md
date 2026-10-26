# D025 — Banner-safe OpenSSH diagnostics on Windows

Date: 2026-10-26 · T013/T014. State: **verified against Windows OpenSSH 9.5p2 and disposable Linux fixture; independently reviewed**.

## Finding

OpenSSH server authentication banners are written as raw bytes to stderr at normal log levels. Capturing stderr and looking for the client's `Authenticated ... using "publickey"` line is unsafe because banner text can imitate that line and host-key diagnostics. Broad `LogVerbose` file patterns also expose pre-identification diagnostics that include server-controlled text. Windows OpenSSH's `-E` file is unavailable for live reads while its writer is open, and `-E` named-pipe output was empty in the local experiment.

## Evidence

Use `LogLevel=QUIET` and exact `LogVerbose` function patterns for `sshconnect.c:verify_host_key()`, `sshconnect.c:check_host_key()`, and `sshconnect2.c:ssh_userauth2()`. `local_diagnostic_message` also checks the local source path, function, line, and process-ID shape before reading a marker. The Windows 9.5p2 capture shows the expected source-qualified lines. A loopback proxy injects forged fingerprint and auth lines before the SSH identification string, then forwards the actual SSH stream. The elevation integration test passes through this proxy, confirms none-auth remains denied, and verifies that no sudo password reaches the fixture sentinel. It also covers fresh sudo, timestamp preservation, key rotation, expiry, cancellation, cleanup, and redacted audit. The shared SFTP/files integration and workspace check pass. No real host or owner credential was used.

## Consequence

Use quiet mode and exact source-function patterns anywhere SSH stderr is parsed as local auth or host-trust evidence. Require the local source-qualified line shape in parsers. Keep OpenSSH's strict host checking pinned to the verified host key. Preserve the pre-auth gate and separate remote sudo prompt output from local diagnostics. The pattern syntax is documented in the [OpenSSH ssh_config manual](https://github.com/openssh/openssh-portable/blob/master/ssh_config.5); relevant message sources are in [sshconnect.c](https://github.com/openssh/openssh-portable/blob/master/sshconnect.c) and [sshconnect2.c](https://github.com/openssh/openssh-portable/blob/master/sshconnect2.c). Do not broaden verbosity or trust remote banner text for authentication decisions.

## Follow-up

The implementation and controlled fixture are in `crates/atlas-core/src/ssh.rs`, `crates/atlas-core/src/ssh/conversation.rs`, `crates/atlas-core/src/sftp.rs`, `crates/atlas-core/tests/elevation.rs`, and `scripts/linux_fixture.py`. Independent review passed. T013 still needs native UI acceptance; T014's actual SFTP/files regression was rerun successfully. See [D023](D023-windows-ssh-log-sharing.md) for the Windows logfile experiments.
