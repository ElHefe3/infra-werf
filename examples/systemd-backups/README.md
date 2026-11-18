# Systemd backups — reference plugin

A plugin for a real need: knowing whether your servers' backups ran. It suits backups that run as systemd services, usually started by timers (restic, borgmatic, rsync scripts and the like), whatever tool they use. It reads everything with `systemctl` through your own SSH access. It needs no root, never sees a backup repository or its password, and changes nothing.

It is built only with InfraWerf's public plugin API (v2) and is the example the [plugin documentation](../../docs/plugin-api/README.md) points to for packs and jobs together.

## What it adds

| Contribution | Where | What it reads |
| --- | --- | --- |
| Failing services | Plugin data tab, as a list of workloads | System services that failed or are stuck restarting: `systemctl list-units --all --type=service --state=failed,auto-restart …` |
| Failing user services | Plugin data tab | The same for the SSH account's own user services. |
| System state | Plugin data tab, and a badge on the server's card | `systemctl is-system-running`. `degraded` means some unit failed. |
| Last result | Plugin data tab, for a backup you choose | The service's last `Result`. `success` is healthy. |
| Last finished | Plugin data tab, for a backup you choose | When its last run ended. |
| Restarts since started | Plugin data tab, for a backup you choose | How often systemd restarted it. A rising number means a restart loop. |
| Started by | Plugin data tab, for a backup you choose | The timer that starts it. |
| Check backups | A job in the Plugin data tab | Finds every system service with "backup" in its name in the server's saved inventory. For each, it reads the last result, last finish and restarts, then writes one line per backup and how many are failing. You can run it or set a schedule for it. |

## Install and allow

1. In InfraWerf, **Settings → Plugins → Install a plugin**, from this folder (or a `.zip` of it).
2. On the plugin's page, read the commands, choose **Read-only**, and enable it.
3. For each server, allow **read saved state**, **use your SSH access** and **run a read-only command**.
4. Open the server's workspace, refresh its inventory, and use the **Plugin data** tab. To keep the overview current, set a schedule for **Check backups** on the plugin's page.

Read-only is enough: the plugin never asks for a change.

## The job's source

`job.wasm` is built from the Rust crate in [`../systemd-backups-job`](../systemd-backups-job/src/lib.rs). The crate sits outside this folder, so installing the plugin copies only what it runs. To rebuild:

```powershell
rustup target add wasm32-unknown-unknown
mise run exec -- python -I scripts/build_example_job.py
```

`--check` confirms that the committed module matches its source. The build replaces local paths with neutral ones, so it gives the same bytes on every PC.

## Limits

Each of these is a gap in the plugin API, recorded as a ticket ([F26](../../agent/features/F26-plugin-api-gaps.md)):

- **Not only backups in the failing list.** A pack argument cannot contain `*`, so the plugin cannot ask for `*-backup.service` alone (T054).
- **No next run.** A parameter cannot become `<name>.timer`. User-scope backup workers cannot be chosen at all (T055).
- **One backup at a time in the Plugin data tab.** Only the last command result is kept; the job's log has them all (T056).
- **"Finished" even when backups failed.** A job cannot flag findings on the server's card. There is also no overview across servers (T057).
