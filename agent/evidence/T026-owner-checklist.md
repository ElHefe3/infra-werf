# T026 — Windows desktop owner acceptance checklist

Date: 2026-10-27
Feature/ticket: F12 / T026
Revision: T026 verification based on preparation commit `f7bedb1`; application binary built from `7f9ba84`.
Result: technical preflight passed; owner acceptance is pending.

This is a preparation checklist, **not acceptance evidence**. T026 stays in progress until the owner reviews the installed app and accepts the desktop cutover.

## Already proven and reusable

- T018–T024 have accepted native Windows and controlled-fixture evidence for SSH trust/session handling, Files and protected placement, transfers, Terminal, health, workload inspection/control, integrated recovery and lifecycle. Links are in [the checkpoint](CHECKPOINT.md).
- T025 installed the current-user Windows x64 release, ran the installed binary through all nine desktop smoke result groups, checked same-version update and silent uninstall, and reinstalled the release. See [T025 acceptance](T025-acceptance.md).
- The installer is unsigned. It is installed at `%LOCALAPPDATA%\Atlas`; user data is stored separately under `%APPDATA%\dev.atlas.workspace`.

## Data preservation preflight

- The original `data/atlas.sqlite3` still has the saved baseline SHA-256. Keep it unchanged as the Python fallback.
- The default app-data directory already contains `atlas.sqlite3` and a `legacy-recovery-*.sqlite3` file dated 2026-10-25. Their contents were not opened or altered for T025.
- Do not repeat the initial migration over the existing native store. Do not select **Delete app data** in the uninstaller. Keep both the native store and recovery snapshot while acceptance is pending.
- Read-only `PRAGMA integrity_check` returned `ok` for the Python reference, native store and legacy recovery snapshot. The recovery snapshot's logical tables match the reference exactly.
- The native store has the same four server records, IDs, map fields, edges and events as the reference. Its reports match the Rust allowlisted inventory projection: **504 services and 36 containers**. This is 540 total workloads. The intentional allowlist omits legacy `warnings`, service `boot`/`restart` and container `restart` fields; the synthetic migration proof tests this redaction.
- The installer itself does not import the Python database. The technical copy and recovery checks pass; owner must still confirm the installed app displays the intended map before making it primary.

## Owner review on the Windows PC

- [ ] Open the installed Atlas app and confirm the map and inventory are the expected owner data. If the map is empty or unexpected, stop and keep the Python reference as the working copy.
- [ ] Confirm app close/reopen leaves the existing map/layout intact. Existing synthetic edit-persistence proof can be reused; do not make a real-data edit solely for this checklist.
- [ ] If current LAN/VPN access is available, verify strict-trust SSH to `lab-server-2`, `lab-server-3` and `lab-server-4`. Do not attempt `lab-server-1`; it is corrupted, disconnected and powered off.
- [ ] If live targets are unavailable, retain the accepted loopback-fixture proof and record the live check as unavailable. Use fixtures for file round-trip, Terminal, health, workload and service-control actions; do not mutate production service state for acceptance.
- [ ] Confirm Lock clears protected session results and that an SSH or host-trust failure leaves the session locked. Existing fixture/native proof may be reused; record only additional owner observations.
- [ ] Owner confirms the installed desktop is accepted as primary while retaining `data/atlas.sqlite3` and the native recovery snapshot as fallbacks. Any later retirement is a separate deliberate decision.

## Tool limitation during preparation

The Windows `node_repl` and `cua_repl` computer-use helpers both failed to initialize with `failed to write kernel assets: The system cannot find the path specified (os error 3)`. No app UI action was taken in this continuation. This is a tooling limitation, not an Atlas failure; the owner must provide the actual-app observation or the UI bridge must recover before T026 can close.

The live SSH read-only checks for hosts 02–04 also could not authenticate in this shell because the configured agent had no identities loaded; see [D043](../discovery/D043-current-ssh-agent-has-no-loaded-identities.md). This result is inconclusive about host availability.
