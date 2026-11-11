# F23 — Sandboxed plugin views

<!-- atlas-feature: {"id":"F23","scope":"S12","depends_on":["F20","F22"],"deferred":true} -->

## Work packet

**Outcome:** A plugin can add its own panel or whole workspace, running isolated from the InfraWerf interface and able to do only what its grants allow.

**User journey:** Enable a plugin with a view → its tab appears → it shows data and offers actions through the granted capabilities only.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F20](F20-plugin-permissions.md), [F22](F22-plugin-install-integrity.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: Builds on the isolated window proven for remote applications, where content cannot invoke InfraWerf commands. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Deferred until the owner activates S12.

## T047 — Isolated plugin view with a message API

<!-- atlas-ticket: {"id":"T047","feature":"F23","status":"deferred","depends_on":["T045"],"evidence":null} -->

### Outcome and included work

Plugin web content runs in an isolated webview with a narrow, versioned message API to read granted observations and request granted actions.

### Exclusions

Access to Tauri IPC, the main window's DOM, the filesystem or arbitrary network origins; plugin content in the main window.

### Implementation and contracts

Serve plugin assets from a per-plugin origin with a strict content security policy. Every message is checked against the plugin's grants in the core. The API is part of API v1 and documented in the author guide.

**Ticket dependencies:** [T045](F22-plugin-install-integrity.md).

### Acceptance criteria

- [ ] A fixture view renders and reads only the observations its grants allow.
- [ ] Attempts to reach IPC, other origins or ungranted data fail and are logged.
- [ ] Closing, Lock and the kill switch tear the view down.
- [ ] The message API is documented and versioned.

### Verification

Hostile fixture view attempting each escape; native journey; independent review of the isolation.

### Evidence and closure

None yet. The ticket is deferred until the owner activates S12.

### Stop conditions

Stop if isolation from IPC cannot be proven on Windows.
