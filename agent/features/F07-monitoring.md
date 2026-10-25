# F07 — Truthful host/service checks and freshness-aware inventory without leaking protected observations.

<!-- atlas-feature: {"id":"F07","scope":"S05","depends_on":["F06","F03"],"deferred":false} -->

## Work packet

**Outcome:** Truthful host/service checks and freshness-aware inventory without leaking protected observations.

**User journey:** Configure checks → observe healthy/degraded/unreachable independently → fail or age a check → inspect authenticated inventory.

**Scope:** [S05](../scopes/S05-observe-operate.md). **Feature prerequisites:** [F06](F06-ssh-elevation.md), [F03](F03-react-map.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D004](../discovery/D004-migration-baseline.md).

## T019 — Implement bounded HTTP/TCP health checks

<!-- atlas-ticket: {"id":"T019","feature":"F07","status":"todo","depends_on":["T003","T007","T014"],"evidence":null} -->

### Outcome and included work

Observe host reachability and service health separately with explicit timestamps and failures.

### Exclusions

Agent deployment, Internet scanning, performance benchmarking or arbitrary network reconfiguration.

### Implementation and contracts

Use cancellable periodic HTTP/TCP checks with configured endpoints, bounded timeout/concurrency and sensible backoff. Show last check/success and stale/unknown states; only approved baseline results persist. Editing graph edges does not create probes or change networking automatically.

**Ticket dependencies:** [T003](F02-domain-persistence.md), [T007](F03-react-map.md), [T014](F06-ssh-elevation.md).

### Acceptance criteria

- [ ] Reachable host plus failing service, unreachable endpoint and missing/old observation are distinct states.
- [ ] Check cancellation, timeout/backoff and endpoint validation prevent runaway scheduling or fabricated success.
- [ ] Both clients receive consistent timestamped health updates without background protected collection.

### Verification

Deterministic fake HTTP/TCP endpoints and clock-controlled scheduling checks; one healthy-to-failed/stale UI journey.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T019, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record unavailable approved endpoints as setup/target blockers; do not broaden probes or mislabel unknown as down.

## T020 — Refresh approved inventory through SSH

<!-- atlas-ticket: {"id":"T020","feature":"F07","status":"todo","depends_on":["T005","T013","T019"],"evidence":null} -->

### Outcome and included work

Collect explicitly approved inventory through authenticated SSH with honest origin and freshness.

### Exclusions

Custom agents, expanded inventory grants or storing all command output for later display.

### Implementation and contracts

Reuse the baseline allowlist and protected-session boundary. Merge observations without changing owner names/layout/relationships. Distinguish source collection time from UI/coordinator refresh; surface partial inventory and collection failures.

**Ticket dependencies:** [T005](F02-domain-persistence.md), [T013](F06-ssh-elevation.md), [T019](F07-monitoring.md).

### Acceptance criteria

- [ ] Approved inventory updates keep stable host/workload identity and original source times; partial/stale reports remain explicit.
- [ ] Unapproved process/log/configuration data is not silently cached or collected by periodic overview refresh.
- [ ] Authentication/expiry/failure leaves the last approved observation timestamped, without inventing live CPU/disk values.

### Verification

Allowlist/merge/freshness tests with partial and malicious reports; one authenticated refresh journey using a controlled fixture.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T020, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Pause collection fields needing owner classification; do not conceal the approval gap behind a locked UI.
