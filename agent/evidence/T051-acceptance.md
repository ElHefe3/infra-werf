# T051 acceptance — reference plugin for a real homelab need

Date: 2026-11-18. Feature: F25 / S12. Ticket: T051.
Revision: 7ae8b6d
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64.

## The plugin

[`examples/systemd-backups`](../../examples/systemd-backups/README.md) answers one question: did the backups run? The owner asked for it to be designed from their saved server map. The map showed that their backups run as systemd services named `lab-<app>-backup.service` on several servers, and that some had failed or were stuck restarting at the last inventory.

- **Pack (plugin API v2).** Read-only `systemctl` commands, no root:
  - failing and restart-looping services, as a workload kind, for system and user scope;
  - the systemd state, with a badge on the server card;
  - for a chosen backup service: its last result (as a health check), last finish, restart count and timer.
- **Job.** "Check backups" is a Rust module compiled to WebAssembly (source in `examples/systemd-backups-job`). It finds every system service with "backup" in its name in the saved inventory, reads each through the plugin's own commands with `run_command`, and logs one line per backup. You can run it now or on a schedule.
- **Build.** `scripts/build_example_job.py` builds the module reproducibly and replaces local paths. `--check` confirms the committed module matches its source.

Owner decisions (2026-11-18):

- The plugin lives in `examples/`.
- It uses a pack plus a scheduled job.
- Its purpose was taken from the server map.
- The Rust WebAssembly target was installed for the job.
- Acceptance: Claude drives a branch build on a copy of the owner's data while the owner watches.

## Checks on 7ae8b6d

| Check | Result |
| --- | --- |
| `infrawerf-plugin check examples/systemd-backups` | valid for plugin API v2, 9 contributions |
| Sandbox test `the_systemd_backups_job_checks_each_backup_service_and_reports_the_failing_ones` | pass. A fake host with three backups (one fine, one failed, one restarting) and three decoys, which takes workload IDs as InfraWerf does; 10 host calls; the expected four log lines |
| `build_example_job.py --check` | the committed module matches its source |
| Native suite `plugin-backups` (`scripts/verify_native_plugin_backups.mjs`) | pass, against three synthetic backups in the Linux fixture (failed, restarting, healthy) |
| `mise run check`, plugin job and manifest tests | pass |

The `plugin-backups` suite, on a release build:

1. **Install and review.** It installed the plugin from its folder through the normal install path, disabled until reviewed. The plugin page showed every exact `systemctl` command and the job module. It was enabled read-only.
2. **Pack commands.**
   - The failing list showed the failed and the restarting backup, but not the healthy one.
   - The system state read `degraded`, and the badge showed it as unhealthy.
   - The last result read `exit-code` for the failed backup and `success` for the healthy one.
3. **The job.** It logged "3 backups checked, 2 failing" with one line per backup. A 5-minute schedule then ran it unattended at read-only trust, with the same result.
4. **Nothing changed.** No workload control ran, nothing was refused, and each run is in the activity log.

## On the owner's lab

Driven by Claude while the owner watched, on 2026-11-18.

- **Build and data.** The branch's release build ran on a copy of the owner's database (`ATLAS_NATIVE_DB`), with the owner's normal OpenSSH setup. The owner's real database was byte-identical before and after.
- **Install and permissions.** The plugin was installed through the normal install path and enabled read-only. It was allowed (read saved state, SSH, read-only command) only on the three servers the owner named: `lab-server-2`, `-3` and `-4`. `lab-server-1` was never opened.
- **Per server**, after a fresh inventory, the three list and state commands and the job ran. All of it took 35 seconds, and no request was refused.

| Server | Systemd state | Failing services | Backups checked by the job |
| --- | --- | --- | --- |
| `lab-server-2` | running (healthy badge) | none | none: no backup services there |
| `lab-server-3` | degraded (unhealthy badge) | 2, both backups: one failed this morning, one in a restart loop (396 restarts) | 5, 2 failing. The failing two match the list; the other three succeeded overnight. |
| `lab-server-4` | degraded (unhealthy badge) | 1 system and 1 user service, neither a backup | 1, succeeded overnight |

The overview matched the inventory taken three days earlier: the same two backups were failing or restarting then. Names of the owner's services are not recorded here.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| The plugin installs through the normal flow and shows real data from the owner's servers | Installed through the normal install path, reviewed and enabled read-only. It showed current backup results from three of the owner's servers, as above. The owner watched the run and accepted the plugin on 2026-11-18 ("Yes, accept"). This was the branch's release build on a copy of the owner's data, because the owner's installed release predates plugins. |
| It uses only documented API v1 features | It uses only the documented public API: manifest, pack commands and extractors, a workload kind, a badge, and a job with `workloads` and `run_command`. These are plugin API v2 and sandbox contract 1; the criterion predates v2, which is a strict superset of v1. No internal command or private access was used. |
| API gaps found while building it are recorded as tickets | Five deferred tickets in new scope S13 / feature F26: T054 patterns in arguments; T055 composed and user-scope parameters; T056 a result per workload; T057 job findings and an overview across servers; T058 clear errors for a module's command. |

## Found while building it

- **A parameter's value is a workload ID**, not a name. The first job passed names and every read was refused as `not_granted` (T058). Fixed in the job; the parameter rule is now in [plugin logic](../../docs/plugin-api/logic.md). The sandbox test's fake host now refuses names.
- **The module contained the builder's Windows user path.** Rust embeds source paths in panic messages. `build_example_job.py` now replaces them. `public_repo.py` scans binary files too, for personal paths and the owner's terms; the public dry-run copy's history is still clean.
- **Linux fixture scripts without a `.sh` extension** were not kept at LF by `.gitattributes`, so a clone with `core.autocrlf=true` would have broken them. All fixture scripts are now LF, and `.wasm` is binary.
- **The fixture image is not rebuilt by the full native run.** After changing a fixture script, rebuild it with `mise run test:inventory-native` or the `podman build` in `scripts/atlas.ps1`. This is unchanged and noted here.
- **The first attempt to drive the owner acceptance stalled** before installing anything; the cause is not known. The second attempt attached to the running app and completed. Nothing was installed or allowed in the stalled attempt.

## Notes

- **Which backups the job finds.** The job reads the saved inventory, so a backup added since the last inventory appears after the next refresh.
- **Review.** No separate review pass was run.
