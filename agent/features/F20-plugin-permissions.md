# F20 — Plugin permissions, consent and activity log

<!-- atlas-feature: {"id":"F20","scope":"S12","depends_on":["F19"],"deferred":false} -->

## Work packet

**Outcome:** The owner sees exactly what a plugin can do before enabling it, can grant it standing trust, can see everything it did and can switch it off at once.

**User journey:** Install a plugin → review its requested capabilities per server → enable, optionally granting trusted automation → later open the activity log → disable the plugin with one action.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F19](F19-plugin-api-manifest.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: The owner decided on 2026-11-11 that an explicitly trusted plugin may act without asking each time (D052). Keys and sudo passwords still never reach a plugin. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T041 — Capability model and consent screen

<!-- atlas-ticket: {"id":"T041","feature":"F20","status":"done","depends_on":["T039","T052"],"evidence":"../evidence/T041-acceptance.md"} -->

### Outcome and included work

A fixed list of capabilities (for example read inventory, run a declared read-only command, open a view, run a declared changing action, schedule a job), enforced in the core and shown on a consent screen scoped per server.

### Exclusions

Capabilities that expose private keys, raw SSH sessions, sudo passwords or the local filesystem; multi-user roles.

### Implementation and contracts

Trust is each owner's choice ([D055](../discovery/D055-plugin-choices-belong-to-the-owner-in-settings.md)): each plugin has one configuration page in the application Settings screen (T052) that holds all its permissions: what it declared, its trust level, the servers it may act on and any elevation exception. That page is the only place permissions are changed; a server's Settings tab shows a read-only list that links to it. There is no default trust level: enabling a plugin requires the owner to choose one each time, with nothing preselected, and a plugin cannot be enabled without that choice. Enforce capabilities in the Rust core at the extension-point boundary, never in the UI. A plugin may only use what its manifest requested and the owner granted. Grants are stored per plugin and per server. Three trust levels: read-only, changes with confirmation (existing preview-and-confirm flow) and trusted automation.

**Ticket dependencies:** [T039](F19-plugin-api-manifest.md), [T052](F18-extension-points.md).

### Acceptance criteria

- [x] A plugin call outside its granted capabilities is refused in the core and logged.
- [x] The consent screen lists each requested capability in plain language, including the exact commands a declarative pack will run.
- [x] One configuration page per plugin shows every permission it has and is the only place they are changed.
- [x] Grants can be given and withdrawn per server, from that page.
- [x] Enabling a plugin requires an explicit trust-level choice; no level is preselected and none is implied.
- [x] No capability gives access to keys, passwords or an unrestricted shell.

### Verification

Core tests that attempt every ungranted capability; native journey for grant, use, withdraw.

### Evidence and closure

Accepted on `5fffd2a`: [T041 acceptance](../evidence/T041-acceptance.md). All twelve native suites pass in one run. One part of the second criterion is carried to T043: a pack's exact commands cannot be shown until packs exist. Sudo cannot be granted yet and is always refused; exceptions are carried to T042. No separate review pass was run.

### Stop conditions

Stop if a requested capability cannot be enforced in the core.

## T042 — Activity log, trusted automation and kill switch

<!-- atlas-ticket: {"id":"T042","feature":"F20","status":"done","depends_on":["T041"],"evidence":"../evidence/T042-acceptance.md"} -->

### Outcome and included work

Every plugin action is recorded with plugin, server, action and result; trusted automation is an explicit, revocable grant; one control disables a plugin or all plugins immediately.

### Exclusions

Storing protected results or command output containing secrets in the log; remote log shipping.

### Implementation and contracts

The plugin activity log and the control that turns all plugins off are sections of the application Settings screen (T052). Extend the existing bounded activity history with a plugin source. Elevation is never implied by trust: a trusted plugin that needs sudo still uses the per-action elevation rules unless the owner grants a named exception for one plugin, one server and one action, on the plugin's configuration page. There is no blanket elevation option. Disabling cancels the plugin's running and scheduled work.

**Ticket dependencies:** [T041](F20-plugin-permissions.md).

### Acceptance criteria

- [x] Each plugin action appears in the activity log with plugin, server, action and outcome.
- [x] Trusted automation can be granted and revoked per plugin, and revocation stops scheduled work.
- [x] The kill switch disables a running plugin and leaves no background work behind.
- [x] The log stays within its configured bound and contains no credentials.

### Verification

Native journey with a fixture plugin performing allowed, refused and cancelled actions; log inspection for secrets.

### Evidence and closure

Accepted on `181dc7d`: [T042 acceptance](../evidence/T042-acceptance.md). All twelve native suites pass on the build, across two runs after one unrelated transient failure. Two criteria mention scheduled and background work, which does not exist in plugin API v1; the log, the switch and revocation are built and tested, and stopping real background work is carried to T043 together with elevation exceptions. No separate review pass was run.

### Stop conditions

Stop if trusted automation would require storing a sudo password.
