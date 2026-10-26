# D023 — Windows OpenSSH diagnostics and the pre-authentication gate

Date: 2026-10-26 · T013. State: **Windows logfile route rejected; exact-function diagnostics and both banner spoof regressions pass; independent review passed; native UI remains pending**.

## Finding

T013 must hold its random remote-input gate until system OpenSSH confirms public-key authentication and the pinned host fingerprint. Windows OpenSSH opens its `-E` diagnostics file exclusively: reopening it while active failed with sharing error 32, and pre-opening a shared file handle made OpenSSH fail to open it (`Broken pipe`). A named-pipe `-E` target accepted the process but delivered no records. The first stderr-pipe workaround also had a security gap because OpenSSH writes server authentication banners directly to stderr, where a server could spoof its local DEBUG text.

## Evidence

The Windows `OpenSSH_for_Windows_9.5p2` executable was tested against the disposable loopback Linux fixture. `LogLevel=QUIET` plus exact `LogVerbose` patterns for `sshconnect.c:verify_host_key()`, `sshconnect.c:check_host_key()`, and `sshconnect2.c:ssh_userauth2()` emit only the three local records Atlas consumes. The parser requires each record's source-file/function/line/PID prefix. A loopback proxy sends forged fingerprint and public-key-success lines before forwarding the SSH identification string; none-auth remains denied and no sudo password reaches the fixture sentinel. SSH_MSG_USERAUTH_BANNER spoofing remains suppressed.

The latest `cargo test -p atlas-core --test elevation --locked -- --ignored --nocapture` passed in 26.35 seconds. It opens an SFTP session with a key, removes that key from the isolated account, enables SSH `none` auth, and confirms the pre-identification spoof does not verify the connection or deliver the sudo password. The same run passes fresh-sudo success/wrong-password despite a global timestamp, timestamp preservation, NOPASSWD denial, cancel/replay, host/config/key mismatch, key rotation, expiry, cleanup, and redacted-audit checks. The SFTP/files integration passed in 41.46 seconds, and `cargo check --workspace --locked` passed. No real host or owner credential was used.

An earlier proposal removed the pre-auth gate; automatic review rejected that weakening and the change was not applied. Independent review then found a second pre-identification spoof path in broad source-file filters. After narrowing the filters and adding the proxy regression, bounded re-review passed with no concrete bypass found.

## Consequence

The command gate and separate random sudo prompt remain. OpenSSH's quiet mode excludes server banner bytes; exact local-function diagnostics supply the public-key method and host-key evidence, and the source-qualified parser rejects pre-identification text. The SFTP reader uses the same quiet/scoped diagnostics so its public-key and pinned-host checks remain intact. Password-bearing commands still merge only remote stderr into stdout, leaving local OpenSSH diagnostics on the separate stderr pipe; the random sudo prompt is removed from returned text. See the [OpenSSH ssh_config manual](https://github.com/openssh/openssh-portable/blob/master/ssh_config.5) for the `LogVerbose` pattern format.

## Follow-up

T013 remains `in_progress` until the native Windows confirmation, wrong-password/cancel, lock, and protected-result-clearing journey passes. Independent review is resolved. Keep S03 open until its ticket criteria are evidenced. T019 may proceed through its explicit T014 scope entry; T020 and terminal/service tickets still depend on T013. See [D025](D025-banner-safe-openssh-diagnostics.md) for the scoped diagnostic design.
