# T049 acceptance — plugin actions and scheduled jobs

Date: 2026-11-17. Feature: F24 / S12. Ticket: T049.
Revision: 7a2cb98
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64. Real workload controls ran only in the disposable loopback Linux container over real OpenSSH, sudo included. No real host was touched.

## What was built

- **The decision.** Owner decision [D061](../discovery/D061-plugin-actions-schedules-and-sudo.md):
  - Actions are InfraWerf's own workload controls only.
  - Sudo needs a named exception and the password every time.
  - The owner sets schedules per server.
  - Scheduled changes happen only under trusted automation; anything less is refused and logged.
- **The `job` point** (plugin API v2, extended in place). A contribution carries a `job`: its `module` (a `.wasm` file in the plugin) and up to 16 `actions`, each with an `id`, `title`, `control` (start, stop, restart), `target` (service, container) and `effect`. Breaking a rule is `invalid_job`. The manifest schema, reference, guide and [plugin logic](../../docs/plugin-api/logic.md) document all of it, and a test checks the documents cover every field.
- **Host functions.**

  | Function | Capability | What it does |
  | --- | --- | --- |
  | `workloads` | `read_saved_state` | Returns the server's workloads from its saved inventory. |
  | `run_command` | `run_approved_command` | Runs one of the plugin's own pack commands. |
  | `control` | `control_workloads` | Asks for one declared action on a workload, named or by ID. |

  `control` opens an authenticated session and accepts only a workload the server's own discovery found, of the action's kind. It previews through the same reviewed control as the Services tab, then decides:

  | Situation | Outcome |
  | --- | --- |
  | Read-only, or `control_workloads` not allowed | Refused. |
  | Changes with confirmation | InfraWerf's own dialog: Run waits 1 s, Decline is focused. |
  | Scheduled run below trusted automation | Refused. |
  | Trusted automation | Runs. |
  | Sudo | Needs the exception, an attended run and the password, which is never stored. |

  The answer to the job is the observed outcome or a reason word.
- **Runs.** Every run gets its own thread, the sandbox's limits and a 10-minute deadline. A request waits at most 5 minutes for the owner. InfraWerf keeps the last 50 runs in memory, with logs, result and each action's outcome. Each run and action is in the plugin activity log.
- **Schedules.** They are core-owned and set per server on the plugin's page.
  - Interval: 5 to 1440 minutes, only while InfraWerf is open.
  - The next run is set when a run starts, so a run is never started twice.
  - Three failed runs in a row pause a schedule, and the log says so. Setting it again resumes it.
  - Disable, withdrawing trusted automation and remove delete schedules. Remove also deletes exceptions.
  - With the kill switch on, no schedule is due.
- **Cancelling.** A permission change stops the job runs of that plugin only. The kill switch stops every run. This narrows the coarse cancel carried in from T043.
- **Interface.**
  - The plugin's page lists each action with its control, target and effect, and per server a schedule control and a sudo exception for each action.
  - The Plugin data tab lists the jobs, with Run now, Stop, the latest run, its actions and log, and the schedule's state.
  - The ask dialog is InfraWerf's own and hides plugin views while it is open.
- **Examples.**
  - [`examples/service-summary`](../../examples/service-summary/infrawerf-plugin.json) is a complete read-only job.
  - `scripts/fixtures/job-plugin` is the native fixture: one user-service action, then one sudo action.
  - Both `job.wasm` files are rebuilt from their `job.wat` by a test, which fails if they differ.

## Checks on 7a2cb98

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 158 passed, 2 ignored (need a real SSH fixture) |
| `mise run test:feature -- T049` | pass (job, sandbox, manifest and host tests; desktop crate; contracts; UI) |
| UI tests | pass: 38 of 38 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 7a2cb98

All 18 suites passed in one uninterrupted run of `ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe mise run test:desktop:full`, ending with the new `plugin-jobs` suite (`scripts/verify_native_plugin_jobs.mjs`). That suite sets a schedule minute to 2 s.

## Acceptance mapping

| Criterion | Evidence (native suite `plugin-jobs` unless named) |
| --- | --- |
| An action runs with confirmation when trust is not granted and unattended when it is | <ul><li>Without `control_workloads` on the server, both actions were refused `not_granted` and the fixture's systemctl ran nothing.</li><li>At confirm-changes, the restart waited in InfraWerf's dialog. Run was disabled at first, Decline was focused, and there was no password field for a user service. It ran only after Run, and `fixture-worker.service` became active.</li><li>Declining ran nothing.</li><li>At trusted automation, a scheduled run restarted the worker with no dialog.</li><li>Read-only trust is refused (`trust_too_low`, core test `an_action_is_refused_below_confirm_changes_and_asks_unless_trusted`).</li></ul> |
| A schedule fires while the app is open, is visible, and stops on revoke or disable | <ul><li>A 5-minute schedule fired unattended, shown as "by its schedule" with "Every 5 minutes while InfraWerf is open".</li><li>Withdrawing trusted automation deleted it, and no run started in the next interval.</li><li>Turning all plugins off cancelled a run waiting in the dialog, and pressing Run afterwards ran nothing.</li><li>Another plugin's change left a waiting run alone.</li><li>Disable, revoke, remove, the kill switch, the 5-minute minimum and pausing after three failures are covered by core tests `schedules_are_the_owners_per_server_and_pause_after_three_failures` and `disabling_revoking_trust_or_removing_deletes_schedules_and_exceptions`.</li></ul> |
| An action needing elevation follows the elevation rules or its named exception | <ul><li>Without the exception, the sudo action was refused `no_elevation_exception` before any dialog.</li><li>With the exception for that one action on that one server, InfraWerf's dialog asked for the password, Run stayed disabled until it was typed, and `fixture-stopped.service` started with fresh sudo.</li><li>The password is in neither the database, the activity log nor the job's state.</li><li>Scheduled runs refused it (`sudo_never_unattended`), even at trusted automation with the exception.</li><li>An exception exists only for a declared action of a plugin that requests `elevate` (core test `a_sudo_exception_is_per_declared_action_and_server_and_logged`).</li></ul> |
| Every run and failure is in the activity log | The log held every run (`keeper: done`, `keeper: scheduled, done`, `keeper: cancelled`), every request ("asked you"), every action done, declined or refused with its reason, the schedule and the exception. The plugin's page describes each entry. |

## Not done, and why

- **Jobs while InfraWerf is closed** are excluded by the ticket and D061; they need a separate runtime decision.
- **Scheduled pack commands on their own.** A job can run a pack command with `run_command` on a schedule, at read-only trust. A schedule for a bare pack command, without a job, was not added.
- **A paused schedule natively.** The fixture job always succeeds, so pausing after three failures is proven by a core test, not the native suite.
- **The kill switch and schedules.** The kill switch makes no schedule due, but it does not delete them: they resume when plugins are turned back on. Disable, revoke and remove delete them.

## Size

SIZE_NOTE

## Notes

- **Found while testing.** The first version of the native suite caught a waiting run being cancelled when a different plugin was enabled. Job cancellation is now limited to the plugin concerned; the suite checks it.
- A module cannot easily parse JSON, so `control` also accepts a workload's name, looked up among workloads of the action's kind. An ambiguous name is refused.
- Review: no separate review pass was run for this ticket.
