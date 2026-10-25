# D016 — Real OpenSSH trust and identity verification

Date: 2026-10-25. State: **resolved**. Tickets: T033, T012, T014.

## Finding

Strict-host checking alone needs explicit surrounding options: a configured localhost-auth bypass can skip it, inherited MD5 display breaks SHA256 diagnostic parsing, and OpenSSH initially attempts none authentication even when publickey is preferred. Successful exit/nonce/fingerprint alone does not prove an existing public-key identity authenticated.

Manufacturing a HostKeyAlias also changes nonstandard-port fallback and host-certificate principal verification. Preserve absence; retain explicit aliases. A certificate diagnostic's SHA token can end with a comma.

## Evidence

The bounded reviewer identified these paths in [OpenSSH 9.5 authentication](https://github.com/openssh/openssh-portable/blob/V_9_5_P1/sshconnect2.c) and [host checking](https://github.com/openssh/openssh-portable/blob/V_9_5_P1/sshconnect.c). The [OpenSSH manual](https://man.openbsd.org/ssh_config) documents the relevant options.

Final `test:ssh` passes 13 actual system-OpenSSH cases, including a none-auth server which spoofs a publickey-success stderr line, a CA-signed host cert with a bare hostname principal on a nonstandard port, unqualified fallback and explicit alias. The fixture intentionally enables localhost bypass/MD5; CLI overrides still reject unknown/changed hosts before authentication. Config and both trust files are digest-preserved. [T033 proof](../evidence/T033-acceptance.md) records paths and the actual native journey.

Windows account-name resolution delayed a temporary fixture-key ACL grant by about ten seconds. Granting the current token's numeric SID avoids that domain lookup; only the newly generated fixture file is changed. No administrator/global policy change is needed.

## Consequence

The backend-only connection-check ticket is distinct from authenticated inspection sessions. Native commands accept selected server/operation IDs only, validate their local main-window origin, use strict fixed OpenSSH options and execute one echo nonce. Raw diagnostics consume a bounded budget but never become UI/SQLite output. The native lifecycle lease follows the actual executor through kill/reap/pipe cleanup, not merely the waiting caller's lifetime.

## Follow-up

Actual protected desktop sessions, user-visible expiry/locking and SFTP require their own next vertical slice. Reuse this adapter and guard. Stage desktop/phone roadmap phases coherently rather than repeatedly rebuilding foundations; do not count verification as an open SSH session. Proxy/jump/helper support, real loaded-agent proof, sudo and paired operation integration remain T012–T014/F05/F08 work. Existing trusted config hooks are evaluated by OpenSSH and are not sandboxed by Atlas.
