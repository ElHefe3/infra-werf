# F23 — Sandboxed plugin views

<!-- atlas-feature: {"id":"F23","scope":"S12","depends_on":["F20","F22"],"deferred":false} -->

## Work packet

**Outcome:** A plugin can add its own panel or whole workspace, running isolated from the InfraWerf interface and able to do only what its grants allow.

**User journey:** Enable a plugin with a view → its tab appears → it shows data and offers actions through the granted capabilities only.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F20](F20-plugin-permissions.md), [F22](F22-plugin-install-integrity.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: Builds on the isolated window proven for remote applications, where content cannot invoke InfraWerf commands. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T047 — Isolated plugin view with a message API

<!-- atlas-ticket: {"id":"T047","feature":"F23","status":"done","depends_on":["T045"],"evidence":"../evidence/T047-acceptance.md"} -->

### Outcome and included work

Plugin web content runs in an isolated webview with a narrow, versioned message API to read granted observations and request granted actions.

### Exclusions

Access to Tauri IPC, the main window's DOM, the filesystem or arbitrary network origins; plugin content in the main window.

### Implementation and contracts

Serve plugin assets from a per-plugin origin with a strict content security policy. Every message is checked against the plugin's grants in the core. The API is part of API v1 and documented in the author guide.

**Ticket dependencies:** [T045](F22-plugin-install-integrity.md).

### Acceptance criteria

- [x] A fixture view renders and reads only the observations its grants allow.
- [x] Attempts to reach IPC, other origins or ungranted data fail and are logged.
- [x] Closing, Lock and the kill switch tear the view down.
- [x] The message API is documented and versioned.

### Verification

Hostile fixture view attempting each escape; native journey; independent review of the isolation.

### Evidence and closure

Accepted on `bec7d39`: [T047 acceptance](../evidence/T047-acceptance.md). Owner decision [D059](../discovery/D059-plugin-views-tab-confirm-and-review.md) first. All four criteria are met in the new `plugin-views` native suite with a hostile view. An independent review found one high (WebRTC), three medium and other findings; all but two informational ones were fixed and re-tested. Remaining: WebRTC is held by the guard script only, and a local program can still deny a view service.

### Stop conditions

Stop if isolation from IPC cannot be proven on Windows.
