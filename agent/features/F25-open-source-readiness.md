# F25 — Open-source readiness and reference plugin

<!-- atlas-feature: {"id":"F25","scope":"S12","depends_on":["F19","F21"],"deferred":false} -->

## Work packet

**Outcome:** InfraWerf can be published as open-source software that other people install and extend, with a real plugin that shows the model working.

**User journey:** A stranger finds the repository → installs InfraWerf → installs the reference plugin → reads the guide and writes their own.

**Scope:** [S12](../scopes/S12-extensibility.md). **Feature prerequisites:** [F19](F19-plugin-api-manifest.md), [F21](F21-declarative-packs.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: The owner's instance remains theirs alone; openness is about the software and its plugin ecosystem. Direction and working assumptions are recorded in [D052](../discovery/D052-plugin-platform-direction.md). Activated 2026-11-12 ([D053](../discovery/D053-owner-activates-extensibility.md)).

## T050 — Licence, public repository and contribution rules

<!-- atlas-ticket: {"id":"T050","feature":"F25","status":"done","depends_on":["T040"],"evidence":"../evidence/T050-acceptance.md"} -->

### Outcome and included work

A chosen licence, a public repository free of private lab data, and contribution, security-reporting and plugin-compatibility documents.

### Exclusions

Publishing to app stores; a hosted plugin catalogue; removing the owner's private ops records from their own machines.

### Implementation and contracts

Audit the repository and history for hostnames, addresses, keys and evidence that names the owner's lab before any publication. Decide the licence for the core and state the expectation for plugins. Publishing is an owner action.

**Ticket dependencies:** [T040](F19-plugin-api-manifest.md).

### Acceptance criteria

- [x] The licence decision is recorded and applied.
- [x] A secrets and private-data audit of the tree and history is recorded with its findings resolved.
- [x] Contribution, security-reporting and API-compatibility documents exist.
- [x] The owner has approved publication explicitly.

### Verification

Audit report; a clean clone builds from the public documents alone.

### Evidence and closure

Accepted on `71cbcae`: [T050 acceptance](../evidence/T050-acceptance.md). Owner decision [D062](../discovery/D062-licence-and-public-repository.md): MIT OR Apache-2.0, all history rewritten into a separate public copy, `agent/` published with lab details replaced, security reports through GitHub private vulnerability reporting. The dry-run public copy has 0 problems and a clean clone of it builds. The owner approved publication on 2026-11-18 ("Yes, approve"). Publishing, from `master` once S12 is complete, is the owner's action by [PUBLISHING.md](../PUBLISHING.md); nothing was pushed. No separate review pass was run.

### Stop conditions

Stop before any push to a public remote without explicit owner approval.

## T051 — Reference plugin for a real homelab need

<!-- atlas-ticket: {"id":"T051","feature":"F25","status":"todo","depends_on":["T043","T040"],"evidence":null} -->

### Outcome and included work

One genuinely useful plugin, built only with the public API, used on the owner's lab (working assumption: a backup overview across servers).

### Exclusions

Private access to core internals; anything the public API cannot express.

### Implementation and contracts

Build it as a declarative pack first and add a view or logic only if those tiers are active. Every limitation found becomes a ticket against the API rather than a workaround.

**Ticket dependencies:** [T043](F21-declarative-packs.md), [T040](F19-plugin-api-manifest.md).

### Acceptance criteria

- [ ] The plugin installs through the normal flow and shows real data from the owner's servers.
- [ ] It uses only documented API v1 features.
- [ ] API gaps found while building it are recorded as tickets.

### Verification

Owner acceptance on the installed desktop application.

### Evidence and closure

None yet.

### Stop conditions

Stop if the plugin needs a capability the owner has not approved.
