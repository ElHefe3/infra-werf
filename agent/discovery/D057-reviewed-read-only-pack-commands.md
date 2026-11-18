# D057 — Rule 6 widens to reviewed read-only pack commands

Date: 2026-11-16. Related: S12 / F21 T043; [D052](D052-plugin-platform-direction.md), [D055](D055-plugin-choices-belong-to-the-owner-in-settings.md).

## Owner decision

`SCOPE.md` rule 6 allowed only operations built into InfraWerf. S12 noted that from F21 this would widen "to operations declared by plugins the owner has reviewed and enabled, which still needs a recorded owner decision".

Asked on 2026-11-16 how far to widen it, with three options (reviewed read-only; reviewed read-only plus a tick per command per server; do not widen yet), the owner chose **"Reviewed read-only"**:

> A pack command may run only if the plugin is enabled, you have seen the exact command on its page, and you allowed it on that server. Fixed argument lists, no shell, no sudo, bounded output and time.

## What this means

A command a plugin pack declares may run on a server only when all of these hold. Each is enforced in the Rust core, not in the interface.

1. **Reviewed.** The plugin's page shows the exact argument list of every command before the plugin can be enabled. If the manifest changes afterwards, the plugin is switched off until the owner reviews and enables it again, so a run always uses the text that was reviewed.
2. **Enabled and allowed on that server.** The plugin is enabled, plugins are not all turned off, and the owner allowed both `ssh_session` and `run_approved_command` on that server. Each run is decided against these settings at that moment and is logged.
3. **An argument list, not shell text.** Arguments hold only letters, digits and `_ . , : / = + @ % -`. No shell reads them. Shells, interpreters, sudo and other ways to become another user, network clients and programs whose purpose is to change a system are refused by name.
4. **No sudo.** `elevate` stays refused for every plugin.
5. **Values only from authenticated discovery.** A parameter is filled with the name of a workload the run's own authenticated session discovered. Nothing typed and nothing from the pack can be a value.
6. **Bounded like built-in collectors.** The same broker limits apply: 64 KiB of output and 30 seconds. A run is cancelled, and its process on the server killed, when its permission changes.
7. **Nothing kept.** Raw output never leaves the core. Extracted values are held in memory only, never written to the database.
8. **Only when asked.** A command runs when the owner presses Run. There is no schedule.

## What it does not mean

- **InfraWerf cannot prove a program only reads.** The name list stops mistakes and obvious abuse. `systemctl` can stop a service as well as report on one. The real check is the owner reading the command, which is why it is shown in full and why a changed manifest needs a new review.
- **Not approved here:** commands that need sudo, commands that change a server, scheduled or unattended runs, and a per-command tick. Each would need its own decision.

## Still open

- Scheduled pack runs under trusted automation, and elevation exceptions (one plugin, one server, one named action). Both move to T049, which is where plugin actions and schedules are defined.
- Whether to add the per-command tick later if packs from strangers become common (F22, F25).
