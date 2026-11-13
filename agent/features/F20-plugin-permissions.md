# F20 — Plugin permissions, consent and activity log

<!-- atlas-feature: {"id":"F20","scope":"S12","depends_on":["F19"],"deferred":false} -->

## Work packet

**Outcome:** The owner sees exactly what a plugin can do before enabling it, can grant it standing trust, can see everything it did and can switch it off at once.

**User journey:** Install a plugin → review its requested capabilities per server → enable, optionally granting trusted automation → later open the activity log → disable the plugin with one action.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F19](F19-plugin-api-manifest.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: The owner decided on 2026-11-11 that an explicitly trusted plugin may act without asking each time (D052). Keys and sudo passwords still never reach a plugin. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T041 — Capability model and consent screen

<!-- atlas-ticket: {"id":"T041","feature":"F20","status":"todo","depends_on":["T039","T052"],"evidence":null} -->

### Outcome and included work

A fixed list of capabilities (for example read inventory, run a declared read-only command, open a view, run a declared changing action, schedule a job), enforced in the core and shown on a consent screen scoped per server.

### Exclusions

Capabilities that expose private keys, raw SSH sessions, sudo passwords or the local filesystem; multi-user roles.

### Implementation and contracts

Trust is each owner's choice ([D055](../discovery/D055-plugin-choices-belong-to-the-owner-in-settings.md)): the consent screen and trust level live in the application Settings screen (T052), and per-server grants in that server's Settings tab. A newly enabled plugin starts at the default trust level (proposed: read-only; a project-owner decision). Enforce capabilities in the Rust core at the extension-point boundary, never in the UI. A plugin may only use what its manifest requested and the owner granted. Grants are stored per plugin and per server. Three trust levels: read-only, changes with confirmation (existing preview-and-confirm flow) and trusted automation.

**Ticket dependencies:** [T039](F19-plugin-api-manifest.md), [T052](F18-extension-points.md).

### Acceptance criteria

- [ ] A plugin call outside its granted capabilities is refused in the core and logged.
- [ ] The consent screen lists each requested capability in plain language, including the exact commands a declarative pack will run.
- [ ] Grants can be given and withdrawn per server.
- [ ] No capability gives access to keys, passwords or an unrestricted shell.

### Verification

Core tests that attempt every ungranted capability; native journey for grant, use, withdraw.

### Evidence and closure

None yet.

### Stop conditions

Stop if a requested capability cannot be enforced in the core.

## T042 — Activity log, trusted automation and kill switch

<!-- atlas-ticket: {"id":"T042","feature":"F20","status":"todo","depends_on":["T041"],"evidence":null} -->

### Outcome and included work

Every plugin action is recorded with plugin, server, action and result; trusted automation is an explicit, revocable grant; one control disables a plugin or all plugins immediately.

### Exclusions

Storing protected results or command output containing secrets in the log; remote log shipping.

### Implementation and contracts

The plugin activity log and the control that turns all plugins off are sections of the application Settings screen (T052). Extend the existing bounded activity history with a plugin source. Elevation is never implied by trust: a trusted plugin that needs sudo still uses the per-action elevation rules unless the owner grants a named exception for one plugin, one server and one action, in that server's Settings tab. There is no blanket elevation option. Disabling cancels the plugin's running and scheduled work.

**Ticket dependencies:** [T041](F20-plugin-permissions.md).

### Acceptance criteria

- [ ] Each plugin action appears in the activity log with plugin, server, action and outcome.
- [ ] Trusted automation can be granted and revoked per plugin, and revocation stops scheduled work.
- [ ] The kill switch disables a running plugin and leaves no background work behind.
- [ ] The log stays within its configured bound and contains no credentials.

### Verification

Native journey with a fixture plugin performing allowed, refused and cancelled actions; log inspection for secrets.

### Evidence and closure

None yet.

### Stop conditions

Stop if trusted automation would require storing a sudo password.
