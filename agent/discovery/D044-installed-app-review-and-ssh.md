# D044 — Installed-app review distinguishes the release from the development window

Date: 2026-10-27<br>
State: verified<br>
Affected: T026

## Finding

On the Windows PC, an already-running `target/debug/atlas-native.exe` showed an empty workspace. After closing it and opening the registered installed Atlas app, the saved owner map loaded: four servers, four saved inventories and 540 workloads. The cause of the development window's empty data context was not established. The installed app is the valid surface for T026 owner review; an empty development window is not evidence that the owner store is missing.

The installed app's Inventory view reported 540 saved workloads. A normal close/reopen preserved the same four map cards and summary. Its strict-host-checking SSH flow authenticated the configured existing identity for `lab-server-2`, `lab-server-3` and `lab-server-4`; no persistent session remained open after each check. `lab-server-1` was not attempted because the owner confirmed it is corrupted, disconnected and powered off.

## Evidence

On 2026-10-27, the actual Windows UI was inspected with Computer Use. The installed app displayed 4 servers / 540 workloads, the Inventory screen reported 540, and the same map returned after normal close/reopen. The app reported SSH access verified for `owner@192.168.20.12:22`, `owner@192.168.20.13:22` and `owner@192.168.20.14:22`, each with one configured identity reference and no persistent session. No map/database edits, migration, trust-file changes or production service/file operations were performed. The T026 checklist records the observations.

The separate `ssh-add -l` result in [D043](D043-current-ssh-agent-has-no-loaded-identities.md) remains accurate for that shell context; it did not describe the installed app's OpenSSH result.

## Consequence

Use the installed app for owner data review. Do not initialize or migrate over the existing native store because a development window appeared empty. The owner accepted the installed desktop as primary on 2026-10-27; retain the Python reference and legacy recovery snapshot as fallbacks.

## Follow-up

The technical review and live read-only SSH checks are recorded in [T026 owner checklist](../evidence/T026-owner-checklist.md) and [T026 acceptance](../evidence/T026-acceptance.md). The owner confirmed that the installed desktop should be primary while both fallbacks remain available.
