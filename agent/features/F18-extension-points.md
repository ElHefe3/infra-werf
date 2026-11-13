# F18 — Extension points inside the core

<!-- atlas-feature: {"id":"F18","scope":"S12","depends_on":["F12"],"deferred":false} -->

## Work packet

**Outcome:** The core exposes named extension points, and one built-in feature is delivered through them, so later plugins attach to stable seams instead of hard-wired code.

**User journey:** Open InfraWerf → every workspace tab, server-card badge, collector and operation is listed in a registry → remote applications still works, now registered as a built-in extension.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F12](F12-release-acceptance.md). It also builds on [F17](F17-remote-applications.md) as shipped in 0.1.5; that is not a tracked dependency because T035 is still awaiting closure.

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: Today the 56 desktop commands and the workspace tabs in `ui/main.tsx` are wired directly. Nothing can be added without editing the core. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T037 — Introduce registries for tabs, badges, collectors and operations

<!-- atlas-ticket: {"id":"T037","feature":"F18","status":"done","depends_on":["T026"],"evidence":"../evidence/T037-acceptance.md"} -->

### Outcome and included work

Typed registries in the Rust core (collectors, health checks, operations) and the React UI (workspace tabs, server-card badges, settings sections). Built-in features register themselves at start-up; behaviour is unchanged.

### Exclusions

Loading anything from outside the application binary; any new user-visible feature; changes to SSH, elevation or session rules.

### Implementation and contracts

Define one registration contract per extension point in `crates/atlas-core` and generate its TypeScript type through the existing contract generation. Each entry carries a stable ID, a display name, the capabilities it needs and its owner (`builtin`). Move the existing tabs and inventory collectors onto the registries without changing their commands.

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [x] Every workspace tab and server-card badge is rendered from a registry entry, not a hard-coded list.
- [x] Inventory collectors and health checks are enumerated from a registry in the core.
- [x] All existing native suites pass unchanged, proving no behaviour change.
- [x] Each registry entry declares the capabilities it uses.

### Verification

Existing native verification suites and UI tests, plus a unit test per registry that rejects duplicate IDs and undeclared capabilities.

### Evidence and closure

Accepted on `33474bc`: [T037 acceptance](../evidence/T037-acceptance.md). All four criteria are met and all twelve native suites pass. The record discloses one updated inventory-suite assertion, the review findings and the one limit left for F20.

### Stop conditions

Stop if a registry cannot be introduced without changing an approved security boundary; record a discovery instead.

## T038 — Deliver remote applications as a built-in extension

<!-- atlas-ticket: {"id":"T038","feature":"F18","status":"todo","depends_on":["T037"],"evidence":null} -->

### Outcome and included work

The remote-applications feature (F17) is registered through the extension points only: its tab, its discovery list and its tunnel operations. It proves the seams are sufficient for a real feature.

### Exclusions

New remote-application behaviour; moving the feature out of the repository; any change to the loopback-only forwarding boundary (D051).

### Implementation and contracts

Remove the direct wiring of remote applications from `ui/main.tsx` and the command list, and register the same pieces through T037's contracts. Record every place where the extension points were too narrow and widen the contract rather than bypassing it.

**Ticket dependencies:** [T037](F18-extension-points.md). Converts the remote-applications feature of [T035](F17-remote-applications.md) as shipped.

### Acceptance criteria

- [ ] Disabling the built-in extension removes the Apps tab and its commands with no other effect.
- [ ] The remote-applications native suite passes unchanged.
- [ ] Gaps found in the extension contracts are fixed in the contracts and listed in the evidence.

### Verification

`verify_native_apps` suite and a test that runs the application with the extension disabled.

### Evidence and closure

None yet.

### Stop conditions

Stop if the conversion needs a private back door into the core; that means the extension points are wrong.
