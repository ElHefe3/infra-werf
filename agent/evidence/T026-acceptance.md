# T026 acceptance — Windows desktop owner review and cutover

Date: 2026-10-27  
Ticket: T026  
Revision: Application release built from `7f9ba84`; owner review and installed-app journey performed 2026-10-27; documentation closure follows checkpoint `ae6f186` and the current working tree.  
Platform: Windows x64, installed current-user Atlas release.  
Result: pass

## Acceptance mapping

| Criterion | Evidence | Result |
| --- | --- | --- |
| Owner review on the actual Windows PC covers the desktop core, session/host-trust failures and application lifecycle | The installed app displayed the saved four-server map, four inventories and 540 workloads; Inventory also showed 540. Normal close/reopen preserved the map. Installed-app strict-host-checking SSH succeeded for hosts 02–04 and left no persistent sessions. Existing native/fixture evidence covers T013 authentication/elevation and lock behavior, T018 SSH/SFTP/sudo and protected placement, T019 health, T020 inventory refresh, T021 terminal, T022 inspection/logs and T023 scoped control. The installed-app observations are recorded in the [owner checklist](T026-owner-checklist.md); technical proofs are linked in [S04](S04-acceptance.md), [S05](S05-acceptance.md), [T013](T013-progress.md), [T018](T018-acceptance.md), [T019](T019-acceptance.md), [T020](T020-acceptance.md), [T021](T021-acceptance.md), [T022](T022-acceptance.md) and [T023](T023-acceptance.md). | Pass |
| Cutover preserves data with verified recovery; reference/original backups are retained until acceptance and retirement is deliberate | Read-only integrity checks passed for the Python reference, native app database and legacy-recovery snapshot. The recovery snapshot logically matches the reference; the native store has the expected four servers and allowlisted 540-workload projection. No migration or owner-data edit occurred in the UI review. T024 proves safe-copy migration/restart/rollback; T025 documents installed update/uninstall boundaries. Both Python reference and native recovery snapshot remain available. See [T024](T024-acceptance.md), [T025](T025-acceptance.md), [D042](../discovery/D042-existing-native-store-and-recovery.md) and the [owner checklist](T026-owner-checklist.md). | Pass |
| Roadmap, tickets, evidence and release instructions record shipped scope, deferred work and limitations | T026, F12 and S06 link this record. Desktop core is accepted; mobile/coordinator/APK/pairing remain future S09, and S07/S08 remain separately deferred. Remaining distribution and host-validation limits are below. | Pass |

## Owner decision

On 2026-10-27, the owner approved the installed desktop app as primary: “Just keep this primary.” In context, the Python reference database and native legacy-recovery snapshot remain fallbacks. Neither was retired or deleted. Any later retirement is a separate deliberate decision.

## Limits and retained fallbacks

- The Windows installer and installed executable are unsigned. The WebView2 bootstrapper may need network access when WebView2 is absent; see [T025](T025-acceptance.md).
- The actual installed app was reviewed for map/inventory display, restart persistence and read-only SSH access to hosts 02–04. File, terminal, health and workload-control acceptance uses the accepted isolated native/fixture proofs; no production write, terminal, health or service-control action was performed.
- Saved host observations are not a claim of current service health. `lab-server-1` is corrupted, disconnected and powered off per the owner; it was not contacted.
- Mobile app packaging, pairing and physical-phone acceptance remain future S09 work. Agents and rsync remain deferred S07/S08.
- Retain `data/atlas.sqlite3` as the Python reference and the native `legacy-recovery-*.sqlite3` snapshot. The installed native app is primary; the two stores are recovery/reference fallbacks.

## Rollback and availability

The installed app launches from its registered current-user entry and its app data is separate from the Python reference. Normal close/reopen preserved the map. If the installed store requires recovery, preserve it and restore from the verified legacy snapshot using the documented recovery procedure; keep the original Python database untouched. Do not select **Delete app data** during uninstall. No fallback retirement is part of this acceptance.
