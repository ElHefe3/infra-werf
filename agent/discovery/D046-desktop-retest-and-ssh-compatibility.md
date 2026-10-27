# D046 — Production SSH compatibility and complete desktop regression

Date: 2026-10-27<br>
State: resolved<br>
Affected: post-release desktop fixes, InfraWerf 0.1.2

## Finding

The owner reported generic inventory-refresh and Terminal errors despite successful Settings SSH verification. Four concrete compatibility problems were found:

1. Windows OpenSSH doubles backslashes in its local `Found key in` diagnostic path. Matching that escaped spelling against the configured trust-file spelling failed, so SFTP login succeeded but the protected Terminal/inventory paths had no recovered public host key.
2. Minimal Linux hosts lack the optional `hostname` and `getconf` commands. The collector both depended on them and printed field prefixes separately, so missing tools produced malformed scalar lines. Procfs provides the same approved hostname/CPU metadata.
3. A server clock less than a second ahead of the PC could be rejected by strict no-future timestamp validation. A bounded catch-up wait resolves small leads without accepting a future observation.
4. Server 4's `docker` executable is a Podman compatibility wrapper. Treating it as an additional system Docker engine incorrectly made a complete rootless Podman collection partial.

The broader run found suppressed local changed-key diagnostics, an SSH peer accepting TCP then closing before key exchange, stale fixture call signatures, a scheduler-sensitive 40 ms test deadline, and UI test races/ambiguous alert selectors. These were distinguished from the owner-facing defects.

## Evidence

An explicit read-only diagnostic used synthetic local databases and the owner's existing SSH configuration for hosts 02–04. Initially host 02 verified SSH and opened SFTP, while inventory and Terminal returned `host_trust_failure`. Sanitized protocol diagnostics then identified invalid hostname/CPU fields without printing inventory contents. Host 04's collection markers isolated its Podman/Docker compatibility issue.

After repairs, all three hosts returned inventory status `updated` and successfully connected Terminal; each session was locked afterward. No owner database or remote file/service was changed by these probes. The final [desktop retest](../evidence/DESKTOP-RETEST-acceptance.md) records the complete automated/native regression and data-preservation results.

## Consequence

- Decode only the provenance-checked local diagnostic path, require a complete configured trust-source match before opening it, and retain key algorithm/fingerprint checks. Do not broadly canonicalize arbitrary paths.
- Restore only the required local OpenSSH diagnostic functions while keeping server identification/banner logging disabled. Classify pre-key-exchange closure using trusted local connection evidence and absence of a trusted fingerprint.
- Collect optional hardware fields using safe procfs fallbacks and emit complete protocol lines. Skip a duplicate Docker engine only when Podman exists and Docker positively identifies itself as Podman. Genuine Docker failures remain partial.
- Wait at most five seconds for a small source-clock lead using a monotonic bound, checking cancellation/deadline/session validity during and after the wait. Existing strict timestamp validation remains unchanged.
- Present structured trust/authentication/expiry errors in Terminal and inventory UI instead of discarding Tauri error objects.

## Follow-up

The native test harnesses now accept an explicit installed release executable and can resume a sequential suite. The ordinary desktop launcher opens the registered InfraWerf release rather than a stale debug binary. The old Atlas program, registration and broken shortcuts were removed at the owner's explicit request; the databases were preserved.

Expansion of Windows global `__PROGRAMDATA__` trust-source tokens remains a separate compatibility limitation, with fail-closed behavior. It was not involved in these owner hosts. No global security, clock, SSH configuration or server permissions were changed.
