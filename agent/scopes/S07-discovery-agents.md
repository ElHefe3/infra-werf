# S07 — Deferred discovery agents

<!-- atlas-scope: {"id":"S07","features":["F13"],"depends_on":["S06"],"window":null,"deferred":true} -->

## Outcome

Optional restricted collectors and always-on coordinator deployment, with separately approved device/module/host boundaries.

## Entry and sequence

Owner explicitly activates S07 after core release and records installation/role policy. Always-on host, route and credential decisions are still owner-gated.

Enrol/revoke restricted devices; add approved modules/buffering; only then deploy the optional coordinator to an approved host with recovery proof.

**Scope dependencies:** [S06](S06-core-release.md). **Budget:** deferred; outside the two-window core target.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F13](../features/F13-discovery-agents.md) | T027, T028, T029 | Approve baseline module → enrol restricted device → observe buffered inventory → revoke → verify no privileged bypass. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] Single-use enrolment and revocation cannot grant owner/SSH/sudo privileges.
- [ ] Configured modules, freshness and bounded offline buffering preserve baseline-only retention.
- [ ] Approved deployment/restart/upgrade/rollback/backup-restore and client trust behavior pass without permanent privileged bypass.

## Verification and refinement

Focused identity/module/buffer checks plus a controlled collector and deployment/recovery journey after authorisation.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

## Exclusions and stop boundary

Core delivery budget, automatic installs on all hosts or invented always-on host/privilege policy.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

Optional operational increment only; does not delay declaring the separately accepted core release.
