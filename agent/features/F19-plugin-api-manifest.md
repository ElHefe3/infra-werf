# F19 — Plugin API and manifest v1

<!-- atlas-feature: {"id":"F19","scope":"S12","depends_on":["F18"],"deferred":false} -->

## Work packet

**Outcome:** A public, versioned description of what a plugin is: its manifest, the extension points it may use and the compatibility promise authors can rely on.

**User journey:** An author reads the guide → writes a manifest → runs the validator → sees exactly which rule failed or that the plugin is valid for API v1.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F18](F18-extension-points.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: Other people will write plugins for their own instances, so the contract must be stable and documented from the first public release. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T039 — Define and validate the plugin manifest and API version

<!-- atlas-ticket: {"id":"T039","feature":"F19","status":"done","depends_on":["T038"],"evidence":"../evidence/T039-acceptance.md"} -->

### Outcome and included work

A manifest schema (identity, version, required API version, contributed extension points, requested capabilities) with a validator in the core and a command-line check for authors.

### Exclusions

Executing any plugin content; network installs; signing.

### Implementation and contracts

Choose one manifest format and publish its JSON Schema generated from the Rust types. Plugin IDs are reverse-domain strings; versions follow semantic versioning; the API version is a single integer with a documented compatibility rule. Validation errors name the field and the rule. Unknown fields are rejected so future fields cannot be silently ignored.

**Ticket dependencies:** [T038](F18-extension-points.md).

### Acceptance criteria

- [x] A valid manifest loads; each invalid fixture is rejected with a specific, stable error code.
- [x] The published schema is generated from the same types the core validates with.
- [x] A manifest that requires a newer API version is refused with a clear message.
- [x] The compatibility rule for API changes is written down.

### Verification

Fixture manifests (valid and one per rejection rule) in unit tests; schema generation checked in CI.

### Evidence and closure

Accepted on `8c908f4`: [T039 acceptance](../evidence/T039-acceptance.md). All four criteria are met. Native suites were not run because no application behaviour changed; the record says so. No separate review pass was run.

### Stop conditions

Stop if the manifest needs to grant a capability that SCOPE.md forbids; raise it as an owner decision.

## T040 — Write the plugin author guide and example pack

<!-- atlas-ticket: {"id":"T040","feature":"F19","status":"done","depends_on":["T039"],"evidence":"../evidence/T040-acceptance.md"} -->

### Outcome and included work

An author guide covering the manifest, each extension point, the capability list and the review a user sees, with a minimal example plugin kept working by tests.

### Exclusions

A plugin catalogue or website; marketing material.

### Implementation and contracts

Keep the guide in the repository next to the schema. The example plugin lives under `examples/` and is validated and loaded by the test suite so it cannot drift from the API.

**Ticket dependencies:** [T039](F19-plugin-api-manifest.md).

### Acceptance criteria

- [x] The guide documents every manifest field and extension point in API v1.
- [x] The example plugin validates and loads in an automated test.
- [x] A person who has not seen the code can build the example from the guide alone (recorded walkthrough).

### Verification

Automated load of the example; one recorded cold-read walkthrough of the guide.

### Evidence and closure

Accepted on `8aff7ed`: [T040 acceptance](../evidence/T040-acceptance.md). All three criteria are met. The owner did the cold-read walkthrough on 2026-11-15; five things it showed were fixed. The guide's section on the owner's review is deliberately limited until T041 builds that screen. No separate review pass was run.

### Stop conditions

Stop if the guide has to describe behaviour that is not implemented.
