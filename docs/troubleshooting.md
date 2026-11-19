# Troubleshooting

Find the symptom you see, check the cause, then try the fix. If none of them match, see [Getting help](#getting-help).

- [Install and startup](#install-and-startup)
- [SSH verification](#ssh-verification)
- [Files, Terminal and transfers](#files-terminal-and-transfers)
- [Health](#health)
- [Workloads and controls](#workloads-and-controls)
- [Plugins](#plugins)
- [Data](#data)
- [Getting help](#getting-help)

## Install and startup

**Windows says "protected your PC" when I run the installer.**
The installer is not code-signed yet. Check its SHA-256 against the release notes, then choose **More info → Run anyway**. See [install](install.md#windows-smartscreen-warning).

**InfraWerf opens to a blank window or fails with a WebView2 error.**
Install the Microsoft Edge WebView2 Runtime, then start InfraWerf again. The installer normally downloads it; if that failed, it needs internet access once.

**InfraWerf says it cannot open its data, or the database looks locked.**
Another copy of InfraWerf is probably still running. Close it fully (check Task Manager), then start again. Do not copy the database while InfraWerf is open.

## SSH verification

**Verify SSH access says it cannot resolve the server.**
The connection hostname does not match a `Host` entry in your OpenSSH configuration. Check the spelling, or use the alias you use in a terminal (`ssh <alias>`). Jump hosts, proxy commands and known-host helper configurations are not supported yet.

**The fingerprint is new, or it has changed.**
InfraWerf will not accept it from inside the app. Confirm the fingerprint through your normal process (the one you use when connecting from a terminal, or the server's own console). Only then continue. A changed key can mean the server was rebuilt, or that someone is intercepting the connection; treat it seriously until you know which.

**Verification fails with an authentication error.**
Your public key is not accepted by the server, or your key is not loaded. Check:

- `ssh <alias>` works from a terminal on this PC. If it does not, InfraWerf cannot succeed either.
- Your key is in your SSH agent, or the identity file is referenced in your OpenSSH config.
- InfraWerf uses your existing keys. It never copies or creates them.

**Verification succeeds but Files or Terminal does not.**
Verification only checks the connection. Files, Terminal and transfers open their own authenticated session. Try **Open file session** again and check the message for the exact reason.

## Files, Terminal and transfers

**My file session closed by itself.**
Sessions end after five idle minutes or thirty minutes in total, by design. Open it again. Pressing **Lock** or leaving the tab also closes it.

**A change says its outcome is uncertain.**
The connection dropped after you confirmed the change. InfraWerf closed the session on purpose. Inspect the server (list the directory, or run `ls` in a terminal) to see what happened before trying again.

**I cannot overwrite a file.**
This is intentional. Rename or delete the old file first, or upload under a new name.

**A transfer stopped at the start.**
Check that the source file still exists and that you have permission on the server. Cancel and start a new attempt rather than resuming.

**InfraWerf warns me before closing during a transfer.**
Transfers are still running. Wait for them to finish, or cancel them, before closing.

**The terminal shows nothing or is stuck.**
Press **Lock**, then open Terminal again. If the remote program is waiting for input, try **Ctrl+C**.

## Health

**A server shows "stale" but I can reach it.**
Stale means the saved check is older than the freshness limit. Run the check again. Stale is not offline.

**A server shows "unhealthy" but works in a browser.**
The check uses the address and port you configured. Check the connection hostname, the port and the URL path in the server's settings, and try the same URL from the PC.

**Health is always unknown.**
No check has run yet. Run one from the Health tab.

## Workloads and controls

**Some items show "denied".**
The SSH account does not have permission to read that system's workloads. Check the account's rights on the server, for example membership in the group that can read Docker or the user's systemd session.

**Some items show "unavailable".**
That workload system (systemd user session, Podman or Docker) is not present or not reachable for the account on this server.

**A control asks for a password again.**
That is expected. Every privileged control needs a fresh password. The password is not stored.

**A control says the result is uncertain.**
Check the item on the server before doing anything else. Refresh the Workloads tab once you know its real state.

## Plugins

**A plugin says "Files changed since it was installed".**
Its files differ from what was installed. Nothing it asks for is allowed until you remove it and install it again from a copy you trust.

**A plugin is listed as invalid.**
Its manifest did not pass validation. The page shows the reason code. Plugin authors can check their manifest with the command in the [manifest reference](plugin-api/README.md#checking-a-manifest).

**A plugin's newer version is refused with `newer_api_version`.**
That plugin was written for a newer plugin API than this InfraWerf supports. Update InfraWerf, or ask the author for a version for your release. See [compatibility](COMPATIBILITY.md).

**Install refuses an archive.**
The archive exceeds size, path or file-count limits, contains a link, or has a path that is not allowed on Windows. Nothing is written when an install is refused.

## Data

**My map or servers are missing after an update.**
Updates do not touch your data folder. Check that you are looking at the same Windows account, and that the data folder is `%APPDATA%\dev.atlas.workspace`. Restore from a backup if needed ([data and backup](data-and-backup.md#restore)).

**I want to start over.**
See [reset](data-and-backup.md#reset). Back up first.

## Getting help

Before you report a problem, collect:

- the InfraWerf version (it is in the installer file name, `InfraWerf_<version>_x64-setup.exe`),
- what you were doing and what you expected,
- the exact message on screen, with **any names, addresses, keys and passwords removed**.

Open an issue for a bug. For a security problem, follow [SECURITY.md](../SECURITY.md) instead of posting publicly.
