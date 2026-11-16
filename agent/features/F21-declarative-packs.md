# F21 — Declarative plugin packs

<!-- atlas-feature: {"id":"F21","scope":"S12","depends_on":["F19","F20"],"deferred":false} -->

## Work packet

**Outcome:** A plugin made only of data can add monitoring, inventory, workload kinds, remote-application definitions and map decoration to an instance.

**User journey:** Drop a pack into the plugins folder → review and enable it → new checks, collectors and badges appear for the granted servers.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F19](F19-plugin-api-manifest.md), [F20](F20-plugin-permissions.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: Data-only packs are the main public format: a reviewer can read exactly what a pack runs, and they need no sandboxed code. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T043 — Packs for collectors, health checks and workload kinds

<!-- atlas-ticket: {"id":"T043","feature":"F21","status":"done","depends_on":["T041"],"evidence":"../evidence/T043-acceptance.md"} -->

### Outcome and included work

A pack can declare read-only commands with parsers for inventory and health, and describe additional workload kinds, all run through the existing authenticated session and bounded-output rules.

### Carried in from F20

Recorded when T041 and T042 closed; each must be met here because packs are the first plugin content that can run:

- Show a pack's exact commands on the plugin's page in Settings (T041).
- A scheduled or long-running pack job asks `plugin_authorize` before every run and is cancelled when refused, so revoking trust, withdrawing a permission, disabling the plugin or the kill switch leaves no work behind (T042).
- Elevation exceptions: one plugin, one server, one named action, granted on the plugin's page, off by default (T042, D055).
- Cancelled actions appear in the plugin activity log (T042).

### Exclusions

Commands that change a server; shell features such as pipes or substitution supplied by the pack; unbounded output.

### Implementation and contracts

Commands are argument vectors, not shell strings, with declared parameters validated against authenticated discovery. Parsing uses a small set of built-in extractors (line, table, JSON path). Results are observations with source, time and freshness, like built-in ones, and follow the rule that protected data is fetched on demand and not retained.

**Ticket dependencies:** [T041](F20-plugin-permissions.md).

### Acceptance criteria

- [x] A fixture pack adds a collector and a health check that appear for a granted server.
- [x] A pack command with shell metacharacters or undeclared parameters is rejected at validation.
- [x] Pack observations carry source, time and freshness and obey the retention rules.
- [x] Output and run time are bounded exactly as for built-in collectors.

### Verification

Loopback SSH fixture; rejection fixtures for unsafe commands; native journey showing the new data.

### Evidence and closure

Accepted on `0202e8f`: [T043 acceptance](../evidence/T043-acceptance.md). Rule 6 was widened first by owner decision [D057](../discovery/D057-reviewed-read-only-pack-commands.md). All four criteria are met and proven on a real Linux container by the new `packs` native suite. Not done: workload kinds (moved to T044), scheduled runs and elevation exceptions (moved to T049). No separate review pass was run.

### Stop conditions

Stop if a pack needs protected data kept after the session ends.

## T044 — Packs for remote applications, map badges and relationship kinds

<!-- atlas-ticket: {"id":"T044","feature":"F21","status":"todo","depends_on":["T043"],"evidence":null} -->

### Outcome and included work

A pack can add remote-application definitions for the Apps list, server-card badges driven by pack observations and named relationship kinds for the map.

### Carried in from T043

- A pack can describe an additional workload kind. T043 delivered collectors and health checks only; a kind that can only be listed would have been a table collector under another name, so it belongs with the other additions to existing screens here.

### Exclusions

Changing the loopback-only forwarding boundary; applying network or deployment changes from relationships.

### Implementation and contracts

Remote-application entries reuse F17's discovery contract. Badges bind to a pack observation and a small set of states. Relationship kinds add a label and style only; drawing one still documents intent and applies nothing.

**Ticket dependencies:** [T043](F21-declarative-packs.md).

### Acceptance criteria

- [ ] A fixture pack adds an application to the Apps list that opens through the existing tunnel rules.
- [ ] A badge reflects a pack observation, including stale and unknown states.
- [ ] A pack relationship kind can be chosen when documenting a relationship.

### Verification

Remote-applications fixture and map UI tests with a fixture pack.

### Evidence and closure

None yet.

### Stop conditions

Stop if a pack definition would open a non-loopback forward.
