# S06 acceptance — Core desktop release

Date: 2026-10-27<br>
Scope: S06<br>
Revision: Application release built from `7f9ba84`; owner review on 2026-10-27; documentation closure follows checkpoint `ae6f186` and the current working tree.<br>
Result: pass

## Acceptance gate

| Gate | Evidence | Result |
| --- | --- | --- |
| Shared data/auth/operation regression and safe-copy migration/rollback pass | [T024](T024-acceptance.md) records the integrated synthetic migration, restart/rollback and shared regression, reusing the accepted feature proof for auth, file, terminal and service boundaries. | Pass |
| Pinned mise tasks produce the Windows installer with checksums/signature state and controlled install results | [T025](T025-acceptance.md) records the release build, artifact hashes/signature status, install/launch, same-version update and uninstall/reinstall. | Pass |
| Windows desktop journeys cover installed saved data, restart, strict-trust SSH and core control/lifecycle/auth/recovery proof | The [T026 owner review](T026-owner-checklist.md) records the installed app's 4-server/540-workload view, normal restart and SSH verification for 02–04. Reused actual Windows/native and controlled-fixture evidence covers the broader core journeys; no production file, terminal, health or service-control operation is claimed. | Pass |
| Owner acceptance and limits, restart/availability and rollback are recorded; fallbacks remain available | [T026](T026-acceptance.md) records the owner's approval to keep the installed desktop primary, the retained Python and native recovery fallbacks, and remaining limitations and rollback guidance. | Pass |

## Scope result

The installed Windows desktop release is the owner's primary Atlas app. The original Python SQLite database and native legacy-recovery snapshot remain intact as fallbacks. Desktop S01–S06 are complete. Mobile/coordinator/APK/pairing/physical-phone work remains future S09; agent discovery and rsync remain deferred S07/S08 and are not included in this acceptance.

The installer is unsigned and its WebView2 bootstrapper can require network access. Live read-only SSH verification succeeded for hosts 02–04; host 01 was not contacted because it is corrupted, disconnected and powered off. Production file, terminal, health and workload-control operations were not performed; those acceptance claims rely on the linked isolated Windows/native and fixture journeys.
