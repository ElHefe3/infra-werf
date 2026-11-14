# T052 acceptance — application Settings screen as an extension point

Date: 2026-11-14. Feature: F18 / S12. Ticket: T052.
Revision: 19d1672
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `19d1672` records. The podman machine hosting the Linux fixture was started before the run.

## What was built

- Core: `ExtensionPoint::AppSettingsSection`. Its sections may declare only `read_saved_state` and `edit_map`, so an application setting cannot reach a server. `builtin()` registers two sections: `builtin.app-settings.history-retention` and `builtin.app-settings.ssh-discovery` (33 contributions in total).
- Interface: a gear button in the main navigation opens `SettingsScreen` (`ui/app-settings.tsx`), which draws the bound sections. History retention and "Discover configured servers" moved there with the same commands (`monitoring_history_limit`, `monitoring_discover`) and the same stored value.
- Servers & monitoring keeps only per-server lifecycle and monitoring controls, with a short note and an Open Settings link where the two options were.
- Settings about one server are unchanged and stay in that server's Settings tab ([D055](../discovery/D055-plugin-choices-belong-to-the-owner-in-settings.md)).

## Checks on 19d1672

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 94 passed, 2 ignored (real-SSH fixture tests) |
| `mise run test:feature -- F18` | pass |
| UI tests | pass: 21/21, including the new binding test |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 19d1672

All twelve pass in a single run with no reruns: desktop, ssh, health, inventory, files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps. The monitoring suite now includes the Settings journey and reports it: "application Settings holds history retention and alias discovery; Servers & monitoring no longer does and links to Settings".

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| A Settings entry in the main navigation opens one screen whose sections all come from registry entries | The monitoring suite opens it from the rail and sees both sections by their registered titles. `SettingsScreen` renders only bound contributions, and binding rejects a renderer without a registration. |
| History retention and alias discovery are on that screen and behave as before | The suite changes retention through the select and reads `history_limit` back from the core, restores it, runs discovery from the button and sees the existing status text. The commands and stored value are unchanged. |
| No per-server setting appears on it, and no application-wide setting remains only on another screen | The screen contains only the two registered sections. The suite asserts Servers & monitoring no longer offers the retention select or the discover button. The remaining controls there (lifecycle, enabled, interval, owner-offline, identity review) are per server. |
| Native suites that use the moved options pass, updated only for the new location | No suite used the two options through the interface (they call the commands directly), so none needed updating. The monitoring suite gained the new journey. |

## Notes

- "Import newer reports" in the map footer is an application-wide action, not a setting, and was left where it is. If it should move, it is a small follow-up.
- The Settings screen is deliberately small: two sections. T041 adds the plugin pages to it.
- Review: no separate review pass was run for this ticket. The change is about 150 lines and mirrors the T037 pattern, whose review findings were applied here (tab-style selection by ID is not involved; the map-waits-for-catalogue and monitoring-loaded guards are). The reviewer of T037 and the author are the same agent.
