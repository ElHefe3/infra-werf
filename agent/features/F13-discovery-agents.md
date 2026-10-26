# F13 — Deferred restricted collectors and always-on operation, only after core release and explicit owner activation.

<!-- atlas-feature: {"id":"F13","scope":"S07","depends_on":["F12","F05"],"deferred":true} -->

## Work packet

**Outcome:** Deferred restricted collectors and always-on operation, only after core release and explicit owner activation.

**User journey:** Approve baseline module → enrol restricted device → observe buffered inventory → revoke → verify no privileged bypass.

**Scope:** [S07](../scopes/S07-discovery-agents.md). **Feature prerequisites:** [F12](F12-release-acceptance.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant environment discovery:** [D006](../discovery/D006-separate-ssh-catalog-error.md). Verify the selected remote deployment environment; do not assume this separate Codex-host error is an Atlas application defect.

## T027 — Implement agent enrolment and revocable identities

<!-- atlas-ticket: {"id":"T027","feature":"F13","status":"deferred","depends_on":["T026"],"evidence":null} -->

### Outcome and included work

Add explicitly authorised discovery-agent enrolment using single-use tokens and revocable per-device identities.

### Exclusions

Core-release work, permanent root access or automatic installation on every host.

### Implementation and contracts

Reuse established trust/auth contracts, keep device roles distinct from owner/SSH sessions and define restricted service identity. Installation changes must be reviewed and specifically authorised.

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Only approved single-use enrolment grants the baseline collector role; expiry/replay/revoke are tested.
- [ ] Agent identity cannot invoke protected owner/SSH/sudo operations.
- [ ] Installation/removal and revocation evidence use a controlled target with no credential leakage.

### Verification

Focused enrolment/role/revoke checks plus one controlled collector enrolment.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T027, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Remain deferred until owner activates S07 and approves installation/identity boundaries.

## T028 — Implement approved modules and offline buffering

<!-- atlas-ticket: {"id":"T028","feature":"F13","status":"deferred","depends_on":["T027","T020"],"evidence":null} -->

### Outcome and included work

Collect approved baseline host/service/storage/port metrics with bounded offline buffering and truthful freshness.

### Exclusions

Privileged module exceptions without owner decision, broad file indexing or unlimited history.

### Implementation and contracts

Modules are explicitly configurable and follow the retention allowlist. Bound memory/disk/queue, authenticate reports, preserve source timestamps and reconnect without fabricating continuity or reading protected configs.

**Ticket dependencies:** [T027](F13-discovery-agents.md), [T020](F07-monitoring.md).

### Acceptance criteria

- [ ] Enabled modules collect only approved fields with explicit source/freshness/partial state.
- [ ] Buffer limits/reconnect/order/revoke and version mismatch are handled predictably.
- [ ] Protected files/logs/configuration and privileged operations remain behind separate owner SSH/sudo authentication.

### Verification

Module allowlist and buffer/order failure fixtures plus one disconnect/reconnect journey.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T028, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record new collection scope as a separate owner decision before implementing it.

## T029 — Deploy optional always-on coordinator

<!-- atlas-ticket: {"id":"T029","feature":"F13","status":"deferred","depends_on":["T028","T010"],"evidence":null} -->

### Outcome and included work

Package and deploy the optional coordinator on an owner-selected always-on host with recovery and access proof.

### Exclusions

Assumed host selection, invisible credential migration or infrastructure workload rearrangement.

### Implementation and contracts

Reuse Rust domain/API rather than a second implementation. Choose deployment host, retained data/credentials, network route and scoped privileged exceptions explicitly before changes. Document backup/restore, upgrades, rollback and device trust.

**Ticket dependencies:** [T028](F13-discovery-agents.md), [T010](F05-device-pairing.md).

### Acceptance criteria

- [ ] Owner deployment/credential/module decisions and exact target authorisation are recorded before deployment.
- [ ] Controlled deploy/restart/upgrade/rollback/backup-restore and client reconnect/revocation pass.
- [ ] No new unauthenticated public endpoint or permanent SSH/sudo bypass is introduced.

### Verification

One deployment/recovery/client journey with approved host and retained-data fixtures.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T029, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Remain owner-gated for host/network/credential policy; block deployment rather than invent these decisions.
