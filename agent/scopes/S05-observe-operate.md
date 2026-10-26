# S05 — Observe and operate

<!-- atlas-scope: {"id":"S05","features":["F07","F10","F11"],"depends_on":["S03"],"window":2,"deferred":false} -->

## Outcome

Truthful health/approved inventory and authenticated terminal/service control on Windows.

## Entry and sequence

Full S05 entry requires S03 completion. T019 has a narrow ticket-level entry through completed T014 while T013 remains incomplete; this does not complete S03. T020 and T021–T023 retain their T013 dependencies. S04 is not a dependency; independently ready features can proceed while a blocker is resolved. Use approved probe targets and controlled workloads.

F07 separates host/service observations and SSH inventory. F10 terminal is independent of F11; F11 workload control follows authenticated inventory/inspection.

**Scope dependencies:** [S03](S03-secure-connections.md). **Budget:** window 2.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F07](../features/F07-monitoring.md) | T019, T020 | Configure checks → observe healthy/degraded/unreachable independently → fail or age a check → inspect authenticated inventory. |
| [F10](../features/F10-terminal.md) | T021 | Authenticate → open terminal → type/resize/interrupt → disconnect/lock → reject stale input and reauthenticate. |
| [F11](../features/F11-service-operations.md) | T022, T023 | Select discovered workload → inspect/logs → review action → elevate if needed → observe actual resulting state. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] HTTP/TCP checks expose independent host/service/freshness states with bounded scheduling and explicit failures.
- [ ] SSH inventory updates retain identity/owner edits and do not persist unapproved protected output.
- [ ] Desktop terminal input/resize/backpressure and lifecycle work; stale sessions cannot receive replayed input.
- [ ] Systemd and declared core container operations have correct scope, deliberate action/elevation and actual observed outcomes; logs remain protected.

## Verification and refinement

Deterministic probe/clock fixtures, one authenticated inventory refresh, one actual Windows desktop terminal journey and one controlled reviewed service action. Android UI and journeys remain future S09 work. Broaden regression only for changed shared boundaries.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

## Exclusions and stop boundary

Scanning unknown networks, automatic bulk restarts/deployment, terminal recording, root/container grants or live production restart without explicit authorisation.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

S06 after both S04 and S05 are complete. Preserve roughly the final quarter of window 2 for integration, Windows package and owner-acceptance gates.
