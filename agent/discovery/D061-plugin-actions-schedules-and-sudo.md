# D061 — Plugin actions, schedules and sudo exceptions

Date: 2026-11-17. Related: S12 / F24 T049; [D055](D055-plugin-choices-belong-to-the-owner-in-settings.md), [D057](D057-reviewed-read-only-pack-commands.md), [D060](D060-plugin-sandbox-wasmi.md).

## Owner decisions

D057 allowed only read-only pack commands the owner starts, and said changing commands, sudo and scheduled runs each need their own decision. Asked on 2026-11-17, the owner chose:

1. **Built-in controls only.** A plugin may declare actions that are InfraWerf's own reviewed workload controls: start, stop or restart a service or container from the server's discovered inventory, with the same preview and checks as the Services tab. A plugin cannot declare its own changing commands.
2. **Sudo through a named exception, with the password every time.** The owner may grant an exception for one plugin, one server and one declared action. It is off by default and granted on the plugin's page. Even with it, InfraWerf asks the owner for the sudo password every time and never stores it, so an action that needs sudo is never unattended. Without the exception, it is refused.
3. **The owner sets schedules, per server.** On the plugin's page the owner turns on a schedule for one of a plugin's jobs on one server, every N minutes (at least 5), running only while InfraWerf is open. Each schedule is visible and can be stopped. Revoking, disabling or removing the plugin and the kill switch stop them all. Schedules belong to InfraWerf, not to the plugin.
4. **Scheduled changes only under trusted automation.** If a scheduled run reaches a changing action and the plugin is not at trusted automation, the action is refused and logged. The run continues or stops as the plugin decides. Changes happen unattended only at trusted automation; otherwise only in a run the owner starts and confirms.

## What this means

- A plugin job is a WebAssembly module (D060) declared at the `job` extension point, with the actions it may ask for listed in the manifest: target (service or container) and effect (a sentence for the owner). The plugin's page shows them before it is enabled.
- An action runs only if:
  - the plugin is enabled and allowed `control_workloads` on that server;
  - the workload is one the server's inventory lists and matches the action's target;
  - the trust level admits it.
- What happens next depends on the run and the trust level:
  - **Trusted automation:** an action that needs no sudo runs at once.
  - **Confirm each change, in a run the owner started:** InfraWerf shows its own dialog and runs the action only if the owner presses Run.
  - **Read-only:** the action is refused.
  - **A scheduled run below trusted automation:** the action is refused, as decision 4 says.
- A failure is reported and logged. Nothing retries without bound: a schedule pauses itself after 3 failed runs in a row.

## Not approved here

- Plugin-written changing commands.
- Storing a sudo password.
- Runs while InfraWerf is closed.
- Changes on servers the plugin is not allowed on.
