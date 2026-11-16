# T042 acceptance — activity log, trusted automation and kill switch

Date: 2026-11-16. Feature: F20 / S12. Ticket: T042.
Revision: 181dc7d
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `181dc7d` records.

## What was built

- **Plugin activity log.** Every call `plugin_authorize` answers is recorded with the plugin, the server, the capability asked for and the outcome (allowed, allowed after confirmation, or refused with the reason). Every change the owner makes is recorded too: enable, disable, permissions set, trusted automation revoked, all plugins off, plugins back on. It is its own table, kept to the History retention bound. Plugin entries no longer go into the general history, which removes the T041 concern that a busy plugin could push other events out.
- **What the log can hold.** Identifiers, a plugin's validated name, capability names and fixed outcome words. No output, arguments or free text from a caller: an unknown caller's ID is not stored.
- **Kill switch.** One control in Settings turns every plugin off. While it is on, every call is refused whatever was granted. It is stored, so it survives a restart, and turning plugins back on restores exactly what the owner had set. The command is not behind the usual operation guard, so it works while other work is in flight.
- **Revoking trusted automation.** A plugin at the trusted level can be dropped to "changes with confirmation" without being disabled. Giving more trust still means disabling and enabling again, with an explicit choice.
- **Interface.** A "Plugin activity" section in Settings; the switch at the top of the Plugins section; "Revoke trusted automation" and a recent-activity table on each plugin's page.

## Checks on 181dc7d

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 115 passed, 2 ignored (real-SSH fixture tests) |
| UI tests | pass: 28/28 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 181dc7d

All twelve pass on this build, but not in one uninterrupted run. Desktop, ssh, health and inventory passed; the files suite then failed once at a step this ticket does not touch (the slow-SSH fixture did not report a connection within the test's 5 seconds). Rerun from files onward, the remaining eight passed: files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps. The files suite had also passed eight times in a row alone on this build just before. The desktop suite's plugin journey now also turns all plugins off, reloads, turns them back on, revokes trusted automation and reads the log both on screen and through the command.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Each plugin action appears in the activity log with plugin, server, action and outcome | Core test `every_action_is_logged_with_plugin_server_action_and_outcome_and_nothing_else` checks the exact sequence of seven entries. The native journey finds 11 refused and 5 allowed calls, each with the plugin, a server, an action and a time, and reads them in the Plugin activity table. |
| Trusted automation can be granted and revoked per plugin, and revocation stops scheduled work | Granted by choosing that level when enabling (T041); revoked with the new control, tested in the core and in the native journey. **Scheduled work does not exist in plugin API v1**, so there is nothing to stop yet. What is in place is the rule that makes stopping possible: nothing is cached, every call is decided against what is allowed now, so a revocation takes effect on the next call (core test). Carried to T043: a scheduled job must ask before every run and be cancelled when refused. |
| The kill switch disables a running plugin and leaves no background work behind | The switch refuses all twelve capabilities at once, survives a restart and restores settings (core test; native journey through a reload). **No plugin runs in the background in API v1**, so "leaves no background work behind" is true only because there is none. Carried to T043 with the same rule as above. |
| The log stays within its configured bound and contains no credentials | Core test `the_log_stays_within_the_history_retention_bound` writes 260 entries with the bound at 200 and finds the newest 200. The entry type has no field for output or arguments; a caller-supplied ID containing "password" and a key header is shown not to be stored. The native journey checks each entry's exact set of fields and searches the whole log for credential words. |

## Not done, and why

- **Elevation exceptions.** The ticket describes a named exception for one plugin, one server and one action. A v1 manifest cannot name an action, so there is nothing to make an exception for. Sudo stays refused at every trust level. Carried to T043, where packs declare actions. The stop condition (storing a sudo password) was never approached.
- **Cancelled actions** in the verification text. No plugin action runs long enough to cancel in API v1. Carried to T043.

## Notes

- The log page shows the latest 200 entries; the store keeps up to the History retention setting.
- The trust level a plugin had is kept while all plugins are off, and shown as "Off with all plugins · … when on".
- **A second fix to the Files notice.** The first full run for this ticket failed in file-changes at the "Protected file data has been cleared" step that T038 had tried to fix. The T038 fix missed one ordering: on a remount React renders the new view before the old view's cleanup records the notice. The view now looks once more after mounting. The files suite then passed eight times in a row.
- **Still flaky:** the slow-SSH fixture step in the files suite (one failure in about ten runs today). It is a 5-second wait on a container, not on InfraWerf. Not fixed here.
- Review: no separate review pass was run for this ticket.
