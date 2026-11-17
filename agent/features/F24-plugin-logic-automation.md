# F24 — Sandboxed plugin logic and automation

<!-- atlas-feature: {"id":"F24","scope":"S12","depends_on":["F20","F21","F22"],"deferred":false} -->

## Work packet

**Outcome:** A plugin can run its own logic and automated jobs against servers, inside a sandbox that only reaches the host functions the owner granted.

**User journey:** Enable a trusted plugin → it runs a multi-step job or a schedule → every step is visible in the activity log and stops when trust is revoked.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F20](F20-plugin-permissions.md), [F21](F21-declarative-packs.md), [F22](F22-plugin-install-integrity.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: This is the highest-risk tier and the one that delivers actions and automation. Jobs run while the app is open unless a later decision adds a background or server runtime. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T048 — Choose the sandbox runtime and host functions

<!-- atlas-ticket: {"id":"T048","feature":"F24","status":"done","depends_on":["T046","T044"],"evidence":"../evidence/T048-acceptance.md"} -->

### Outcome and included work

A recorded decision on the sandbox (WebAssembly is the working assumption) and a first host-function set, with a prototype plugin that reads through granted functions.

### Exclusions

Native or unsandboxed script plugins; host functions that return keys, passwords or raw sessions.

### Implementation and contracts

Compare runtimes for Windows and future Android support, resource limits, author experience and size. Host functions map one-to-one onto capabilities from T041. Enforce memory, time and call limits.

**Ticket dependencies:** [T046](F22-plugin-install-integrity.md), [T044](F21-declarative-packs.md).

### Acceptance criteria

- [x] The runtime choice and its trade-offs are recorded as a discovery note.
- [x] A prototype plugin calls a granted host function and is refused an ungranted one.
- [x] Memory, time and call limits stop a runaway plugin without affecting the app.

### Verification

Prototype plus runaway and escape fixtures.

### Evidence and closure

Accepted on `7c49d10`: [T048 acceptance](../evidence/T048-acceptance.md). Owner decision [D060](../discovery/D060-plugin-sandbox-wasmi.md): wasmi. All three criteria are met by core tests with a prototype and 16 runaway and escape fixtures. Starting a module from the app, and host functions that act, move to T049. No separate review pass was run.

### Stop conditions

Stop if no runtime can enforce the limits on Windows.

## T049 — Plugin actions and scheduled jobs under trusted automation

<!-- atlas-ticket: {"id":"T049","feature":"F24","status":"done","depends_on":["T048"],"evidence":"../evidence/T049-acceptance.md"} -->

### Outcome and included work

A sandboxed plugin can run declared changing actions and schedules. Without trusted automation each action uses the preview-and-confirm flow; with it, actions run unattended within the grant.

### Carried in from T042 and T043

Owner decision D057 allows only read-only pack commands that the owner starts. These need their own recorded decision before they are built:

- Scheduled pack runs: each run asks `plugin_begin_run` first and is cancelled, and killed on the server, when refused. The cancel path exists (T043); the schedule does not.
- Elevation exceptions: one plugin, one server, one named action, granted on the plugin's page, off by default (D055). Sudo is refused for every plugin today.
- Removing a plugin (T045) must also remove its schedules. Removal today deletes its files, trust, permissions and install record, cancels pack commands in flight and closes pack tunnels.
- Cancelling is coarse today: any plugin permission change cancels every pack command in flight. Narrow it to the plugin and server concerned once runs can be unattended.

### Carried in from T048

- Declare a plugin's module in the manifest and start runs from the app, through `plugin_sandbox::run` with a `StoreHost`, on the run's own thread.
- Host functions that act (run a pack command, and later files or workloads) each map to one capability, and those that reach a server bring their own time limit: fuel does not bound a host call.
- Measure the size wasmi adds once the app references it.

### Exclusions

Jobs that run while the app is closed (needs a separate runtime decision); implied sudo; changes outside the granted servers.

### Implementation and contracts

Actions are declared in the manifest with their target and effect so the consent screen can describe them. Schedules are owned by the core, not the plugin, so revocation and the kill switch always work. Failures are reported, never retried silently without bound.

**Ticket dependencies:** [T048](F24-plugin-logic-automation.md).

### Acceptance criteria

- [x] An action runs with confirmation when trust is not granted and unattended when it is.
- [x] A schedule fires while the app is open, is visible, and stops on revoke or disable.
- [x] An action needing elevation follows the elevation rules or its named exception.
- [x] Every run and failure is in the activity log.

### Verification

Loopback fixture with a changing action and a schedule; revoke and kill-switch journeys. No real-host testing without owner direction.

### Evidence and closure

Accepted on `7a2cb98`: [T049 acceptance](../evidence/T049-acceptance.md). Owner decision [D061](../discovery/D061-plugin-actions-schedules-and-sudo.md). All four criteria are met by the new native suite `plugin-jobs`, run against the loopback Linux container with real sudo, and by core tests; all 18 native suites passed in one run. Job cancellation is now limited to the plugin concerned. Not done: jobs while InfraWerf is closed (excluded), schedules for bare pack commands, and a native proof of pausing after failures (core test only). No separate review pass was run.

### Stop conditions

Stop if unattended actions cannot be bounded to the granted servers and actions.
