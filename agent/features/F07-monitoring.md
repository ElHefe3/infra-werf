# F07 — Truthful host/service checks and freshness-aware inventory without leaking protected observations.

<!-- atlas-feature: {"id":"F07","scope":"S05","depends_on":["F06","F03"],"deferred":false} -->

Current owner decision (2026-10-26): desktop is the complete release target. Phone/coordinator/APK requirements are preserved in [S09](../scopes/S09-future-mobile.md), separately deferred. All desktop batches are authorised sequentially.

## Work packet

**Outcome:** Truthful host/service checks and freshness-aware inventory without leaking protected observations.

**User journey:** Configure checks → observe healthy/degraded/unreachable independently → fail or age a check → inspect authenticated inventory.

**Scope:** [S05](../scopes/S05-observe-operate.md). **Feature prerequisites:** [F06](F06-ssh-elevation.md), [F03](F03-react-map.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D004](../discovery/D004-migration-baseline.md), [D028](../discovery/D028-health-staleness-is-a-render-time-state.md), [D029](../discovery/D029-windows-temp-sqlite-sandbox.md).

## T019 — Implement bounded HTTP/TCP health checks

<!-- atlas-ticket: {"id":"T019","feature":"F07","status":"done","depends_on":["T003","T007","T014"],"scope_entry":{"S03":["T014"]},"evidence":"../evidence/T019-acceptance.md"} -->

### Outcome and included work

Observe host reachability and service health separately with explicit timestamps and failures.

### Exclusions

Agent deployment, Internet scanning, performance benchmarking or arbitrary network reconfiguration.

### Implementation and contracts

Use cancellable periodic HTTP/TCP checks with configured endpoints, bounded timeout/concurrency and sensible backoff. Show last check/success and stale/unknown states; only approved baseline results persist. Editing graph edges does not create probes or change networking automatically.

**Ticket dependencies:** [T003](F02-domain-persistence.md), [T007](F03-react-map.md), [T014](F06-ssh-elevation.md).

This bounded health-check ticket uses the completed T014 desktop entry from S03 while T013 remains incomplete. That entry does not complete S03; T020 and the terminal/service tickets retain their T013 dependency.

### Acceptance criteria

- [x] Reachable host plus failing service, unreachable endpoint and missing/old observation are distinct states.
- [x] Check cancellation, timeout/backoff and endpoint validation prevent runaway scheduling or fabricated success.
- [x] The Windows desktop shows timestamped host/service results without background protected collection. Android UI and its native polling adapters remain future S09 work.

### Verification

Deterministic loopback HTTP/TCP fixtures, bounded interval/backoff and mid-read cancellation checks; one healthy-to-failed/stale native UI journey.

### Evidence and closure

Passed in [T019 acceptance](../evidence/T019-acceptance.md): focused Rust/UI fixtures, workspace checks, rebuilt Windows app, native healthy→failed/stale/cancel journey against loopback fixtures, and bounded independent review. Mobile remains S09.

### Stop conditions

Record unavailable approved endpoints as setup/target blockers; do not broaden probes or mislabel unknown as down.

## T020 — Refresh approved inventory through SSH

<!-- atlas-ticket: {"id":"T020","feature":"F07","status":"done","depends_on":["T005","T013","T019"],"evidence":"../evidence/T020-acceptance.md"} -->

### Outcome and included work

Collect explicitly approved inventory through authenticated SSH with honest origin and freshness.

### Exclusions

Custom agents, expanded inventory grants or storing all command output for later display.

### Implementation and contracts

Reuse the baseline allowlist and protected-session boundary. Merge observations without changing owner names/layout/relationships. Distinguish source collection time from UI/coordinator refresh; surface partial inventory and collection failures.

**Ticket dependencies:** [T005](F02-domain-persistence.md), [T013](F06-ssh-elevation.md), [T019](F07-monitoring.md).

### Acceptance criteria

- [x] Approved inventory updates keep stable host/workload identity and original source times; partial/stale reports remain explicit.
- [x] Unapproved process/log/configuration data is not silently cached or collected by periodic overview refresh.
- [x] Authentication/expiry/failure leaves the last approved observation timestamped, without inventing live CPU/disk values.

### Verification

Allowlist/merge/freshness tests with partial and malicious reports; one authenticated refresh journey using a controlled fixture.

### Evidence and closure

Passed in [T020 acceptance](../evidence/T020-acceptance.md): focused parser/store checks, rebuilt Windows app, native authenticated success/partial/authentication-failure journey against loopback-only fixtures, and bounded independent review. See [D029](../discovery/D029-windows-temp-sqlite-sandbox.md) for the isolated SQLite test sandbox requirement. Mobile remains S09.

### Stop conditions

Pause collection fields needing owner classification; do not conceal the approval gap behind a locked UI.
