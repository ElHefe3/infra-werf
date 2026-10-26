# T013 fresh per-action sudo blocker

Date: 2026-10-26
Revision: a6706d8 + T013 working tree (not committed)
Result: blocked

## Acceptance mapping

- T013: not passed. The disposable local Linux OpenSSH/sudo fixture reached the wrong-password assertion, but Atlas returned `IoFailure` instead of `Unauthenticated`.
- T013 pre-authentication command gate: preserved. A proposed change that removed the gate was rejected by automatic review and was not applied.
- T013 Windows log capture: blocked. A fixed-stage diagnostic on the active `ssh -E` logfile reported Win32 error 32 (sharing violation). The attempted pre-created regular log with a live reader still failed the focused acceptance case; its failure stage was not independently established.

## Environment and limits

Pinned Windows/Rust toolchain; disposable local Podman Linux image on loopback; synthetic SSH identity and sudo password. No production host, key, known-host file, sudo policy or service was modified. The fixture container was removed by its cleanup path. No successful sudo result is claimed.

## Next step

Keep T013 blocked until live, isolated OpenSSH authentication evidence can be read without weakening the gate. See [D023](../discovery/D023-windows-ssh-log-sharing.md).
