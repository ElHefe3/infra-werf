# T026 — Windows desktop owner acceptance checklist

Date: 2026-10-27
Feature/ticket: F12 / T026
Revision: Application binary built from `7f9ba84`; actual installed-app review and live SSH checks performed on 2026-10-27.
Result: accepted. The installed Windows desktop app is primary; the Python reference database and native legacy-recovery snapshot remain fallbacks.

This checklist records the owner review and decision alongside the technical evidence linked below. The cutover is reversible: no fallback was retired or deleted.

## Already proven and reusable

- T018–T024 have accepted native Windows and controlled-fixture evidence for SSH trust/session handling, Files and protected placement, transfers, Terminal, health, workload inspection/control, integrated recovery and lifecycle. Links are in [the checkpoint](CHECKPOINT.md).
- T025 installed the current-user Windows x64 release, ran the installed binary through all nine desktop smoke result groups, checked same-version update and silent uninstall, and reinstalled the release. See [T025 acceptance](T025-acceptance.md).
- The installer is unsigned. It is installed at `%LOCALAPPDATA%\Atlas`; user data is stored separately under `%APPDATA%\dev.atlas.workspace`.

## Data preservation preflight

- The original `data/atlas.sqlite3` still has the saved baseline SHA-256. Keep it unchanged as the Python fallback.
- The default app-data directory already contains `atlas.sqlite3` and a `legacy-recovery-*.sqlite3` file dated 2026-10-25. Their contents were not opened or altered for T025.
- Do not repeat the initial migration over the existing native store. Do not select **Delete app data** in the uninstaller. Keep both the native store and recovery snapshot as fallbacks after acceptance.
- Read-only `PRAGMA integrity_check` returned `ok` for the Python reference, native store and legacy recovery snapshot. The recovery snapshot's logical tables match the reference exactly.
- The native store has the same four server records, IDs, map fields, edges and events as the reference. Its reports match the Rust allowlisted inventory projection: **504 services and 36 containers**. This is 540 total workloads. The intentional allowlist omits legacy `warnings`, service `boot`/`restart` and container `restart` fields; the synthetic migration proof tests this redaction.
- The installer itself does not import the Python database. The technical copy and recovery checks pass; UI review below confirmed the installed app displays the saved map. The owner accepted making it primary while retaining both fallbacks.

## Windows PC observations — 2026-10-27

- The registered installed Atlas app displayed the existing map: **4 servers, 4 saved inventories and 540 saved workloads**. The Inventory view also reported 540 saved workloads. No owner data was edited.
- Atlas was closed normally and reopened from the installed-app entry. The same four map cards and 4/540 summary returned.
- Strict-host-checking SSH verification in the installed app succeeded for `lab-server-2` (`192.168.20.12:22`), `lab-server-3` (`192.168.20.13:22`) and `lab-server-4` (`192.168.20.14:22`), using one configured identity reference per host. Each check ended without leaving a persistent session. `lab-server-1` was not contacted.
- An already-running `target/debug/atlas-native.exe` window showed an empty workspace. After closing it and launching the registered installed app, the existing saved map appeared. The cause of the development window's empty data context was not established; do not use that window to infer data loss or start a migration.
- No production file, health, terminal, workload or service-control operation was performed. Reuse the accepted fixture/native proof in [T013](T013-progress.md), [T018](T018-acceptance.md) and [T025](T025-acceptance.md) for those actions.

## Owner review on the Windows PC

- [x] Installed-app display reviewed: the saved map and 540-workload Inventory view loaded. Owner confirmation that this is the intended data set is part of the final acceptance below.
- [x] Normal close/reopen preserved the map and 4/540 summary. No real-data edit was made.
- [x] Strict-trust SSH succeeded for hosts 02–04 as listed above. Do not attempt `lab-server-1`; it is corrupted, disconnected and powered off.
- [x] Reused the accepted fixture/native proof for Lock clearing and authentication/trust-failure handling ([T013](T013-progress.md), [T018](T018-acceptance.md)); no production service-control or file operation was used.
- [x] Owner confirms the installed desktop is accepted as primary while retaining `data/atlas.sqlite3` and the native recovery snapshot as fallbacks. Any later retirement is a separate deliberate decision.

Owner decision, 2026-10-27: “Just keep this primary.” In context, this accepts the installed desktop as primary with the Python reference and native recovery snapshot retained. No fallback retirement was requested or performed.
