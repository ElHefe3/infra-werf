# F22 — Plugin install, integrity and updates

<!-- atlas-feature: {"id":"F22","scope":"S12","depends_on":["F20"],"deferred":false} -->

## Work packet

**Outcome:** Owners can install, update and remove plugins knowingly, and a plugin cannot change what it is allowed to do without a new review.

**User journey:** Choose a plugin file or address → InfraWerf verifies it → review → enable; on update, any new capability is shown before the update applies.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F20](F20-plugin-permissions.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: There is no central store in the first release. Plugins come from files, folders or addresses the owner chooses. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T045 — Install, remove and verify plugins

<!-- atlas-ticket: {"id":"T045","feature":"F22","status":"todo","depends_on":["T042"],"evidence":null} -->

### Outcome and included work

Install from a local folder, an archive file or an HTTPS address into a per-user plugins directory, with a recorded content hash and optional author signature; clean removal.

### Exclusions

A hosted catalogue; automatic installs; executing install scripts.

### Implementation and contracts

Unpack with path and size limits into a staging directory, validate the manifest, record the hash, then publish atomically. Verification failure leaves nothing installed. Removal deletes the plugin's files, grants and scheduled work and keeps its activity history.

**Ticket dependencies:** [T042](F20-plugin-permissions.md).

### Acceptance criteria

- [ ] A plugin installs from each of the three sources and appears disabled until reviewed.
- [ ] An archive with path traversal, oversize content or a hash mismatch is refused with nothing left behind.
- [ ] Removal clears files, grants and schedules.
- [ ] The installed hash is shown and re-checked at start-up.

### Verification

Malicious-archive fixtures; native install, restart and remove journey.

### Evidence and closure

None yet.

### Stop conditions

Stop if installation would need to run plugin-supplied code.

## T046 — Updates that re-ask when capabilities grow

<!-- atlas-ticket: {"id":"T046","feature":"F22","status":"todo","depends_on":["T045"],"evidence":null} -->

### Outcome and included work

Updating a plugin compares its requested capabilities with the granted ones; additions require a new review and are never granted silently.

### Exclusions

Background auto-update without the owner's choice; downgrades without warning.

### Implementation and contracts

Keep the previous version until the new one validates. Show a capability difference. An update that only removes capabilities applies without review.

**Ticket dependencies:** [T045](F22-plugin-install-integrity.md).

### Acceptance criteria

- [ ] An update requesting a new capability stays inactive until reviewed.
- [ ] An update with the same or fewer capabilities applies and is logged.
- [ ] A failed update leaves the previous version working.

### Verification

Fixture plugins at two versions for each case.

### Evidence and closure

None yet.

### Stop conditions

Stop if an update could widen access without the owner seeing it.
