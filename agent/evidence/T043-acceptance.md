# T043 acceptance — packs for collectors and health checks

Date: 2026-11-16. Feature: F21 / S12. Ticket: T043.
Revision: 0202e8f
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `0202e8f` records.

Owner decision first: rule 6 was widened to reviewed read-only pack commands on 2026-11-16 ([D057](../discovery/D057-reviewed-read-only-pack-commands.md)). The work below stays inside that decision.

## What was built

- **Plugin API v2.** A contribution at `collector` or `health_check` may carry a `command`: an argument list, up to four parameters, an extractor (`line`, `table` or `json`) and, for a health check, the values that mean healthy. Ten new error codes. Manifests written for v1 are accepted unchanged and mean what they did, including the v1 rule that a health check may use only `network_probe`.
- **Validation.** Arguments may hold only letters, digits and `_ . , : / = + @ % -`. Shells, interpreters, sudo and other ways to become another user, network clients and programs whose purpose is to change a system are refused by name. A `{name}` must be a whole argument and declared; a declared parameter must be used.
- **Running.** A new brokered operation, `builtin.operation.pack-command`, runs the command over the same authenticated, host-pinned session, with the same output and time limits, as the built-in collectors. Each run first passes `plugin_begin_run`, which reads the manifest once and decides every capability the command uses against the owner's settings at that moment.
- **Parameters.** A value is the ID of a workload the run's own authenticated session discovered (a system-scope service or a container). Names, typed text and unknown IDs are refused.
- **Cancelling.** Any change to what plugins may do (disable, permissions, trust revoked, all plugins off) cancels pack commands in flight and forgets pack observations. A cancelled or timed-out command is killed on the server: OpenSSH does not signal a command without a terminal when the client leaves, so the remote shell supervises it and kills it when the connection's input closes.
- **Observations.** Extracted values only, with control characters removed and 200 characters at most; 100 table rows at most. Each carries when it was read, whether it is fresh (15 minutes, the built-in limit), which plugin contribution produced it and the binding of the authenticated SSH profile. Held in memory only.
- **Interface.** The plugin's page shows each exact command, its parameters and how it is read, before the plugin is enabled. Each server has a Plugin data tab listing the commands allowed there, each with a Run button and its latest result. The plugin activity log shows each run as finished, failed or cancelled.
- **Documents.** `docs/plugin-api/packs.md` (new), the reference and guide updated for v2, the example pack `examples/host-basics`.

## Checks on 0202e8f

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 123 passed, 2 ignored (need a real SSH fixture) |
| UI tests | pass: 28 passed |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 0202e8f

All thirteen suites passed in one uninterrupted run of `mise run test:desktop:full`: desktop, ssh, health, inventory, files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps and the new packs suite.

The new thirteenth suite, `packs` (`scripts/verify_native_packs.mjs`), runs the built application against the disposable Linux container over real OpenSSH. Nothing in it is canned: `uname -r` and `df -P -k` return what the container returns, and the test compares them with the container's own answer.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| A fixture pack adds a collector and a health check that appear for a granted server | Native `packs` suite: after enabling the example pack and allowing both permissions on one server, its two collectors and two health checks appear in that server's Plugin data tab and nowhere else. Kernel version shows the container's real kernel; Disk usage shows its real file systems as a table; the parameterised health check is Healthy for one service and Unhealthy for another. Core test `a_pack_command_is_listed_and_admitted_only_where_the_owner_allowed_it`. |
| A pack command with shell metacharacters or undeclared parameters is rejected at validation | Fixtures `unsafe_argument`, `forbidden_program`, `undeclared_parameter` and seven more, each refused with exactly its own code. Test `unsafe_commands_are_refused_however_they_are_written` tries 23 arguments with shell characters, 23 forbidden programs by name and by path, and malformed programs. In the native suite a pack whose command is `sh -c "... | head"` is listed as an unusable folder with `forbidden_program` and is never offered. |
| Pack observations carry source, time and freshness and obey the retention rules | Native suite checks each observation's exact fields: `source`, `source_binding` (the authenticated profile and host key), `observed_at` and `fresh`. It then searches the database and its write-ahead log for the kernel string, the first file system and the health value and finds none, and finds no output in the activity log. Observations are forgotten when permissions change (checked twice). |
| Output and run time are bounded exactly as for built-in collectors | The command goes through the same `OperationBroker::execute` and `ExecutionContext` as the built-in collectors, so the limits are the same code, not a copy. Native suite: `ls -lR /usr` (over 64 KiB in the container) ends as `output_limit` with no value; `sleep 45` ends as `timed_out` after 30 seconds and the process is gone from the container. |

## Carried in from F20: what was met

| Item | Result |
| --- | --- |
| Show a pack's exact commands on the plugin's page | Done, and checked in the native suite before the plugin is enabled. |
| A long-running pack job asks before every run and is cancelled when refused | Done for runs the owner starts: every run is authorised first, and a run in flight is cancelled and killed on the server when permissions change (native suite: cancelled within seconds of "turn all plugins off", process gone). **Scheduled runs do not exist**: D057 covers only runs the owner starts. Moved to T049. |
| Elevation exceptions | **Not done.** D057 keeps sudo refused. Moved to T049. |
| Cancelled actions appear in the plugin activity log | Done: runs are logged as finished, failed or cancelled. |

## Not done, and why

- **Workload kinds.** The ticket's outcome also says a pack can "describe additional workload kinds". None of the four acceptance criteria covers it, and a kind that can only be listed, without inspection or control, would be a table collector under another name. Moved to T044 with the other pack additions to existing screens.
- **Scheduled runs and elevation exceptions**, as above.
- The stop condition (a pack needing protected data kept after the session ends) was not approached: nothing a pack reads is kept.

## Notes

- **The name list cannot prove a command only reads.** `systemctl` can stop a service as well as report on one. The check that counts is the owner reading the command, which is why it is shown in full and why a changed manifest switches the plugin off until it is reviewed again. Recorded in D057.
- **Cancelling is coarse.** Any change to any plugin's permissions cancels every pack command in flight and forgets every pack observation, not only that plugin's. Safe, and simple; it can be narrowed when schedules exist.
- **The server-side kill** sends the command a normal termination signal. A program that ignores it, or its own child processes, could outlive it. The commands a pack may name are simple programs.
- **A health check is judged by what the command printed even when it reports failure**, because `systemctl is-active` prints `inactive` and exits with an error. A collector needs success.
- **A `service` parameter means a system-scope service.** User-scope services are not offered, because the command has no way to say `--user` per value.
- Found while testing: the first version opened the session without the saved inventory, so every parameter was refused; and a cancelled command kept running on the server until it finished by itself. Both fixed before this revision.
- Review: no separate review pass was run for this ticket.
