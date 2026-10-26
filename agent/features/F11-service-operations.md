# F11 — Scoped authenticated workload inspection and deliberate service/container control.

<!-- atlas-feature: {"id":"F11","scope":"S05","depends_on":["F07","F06"],"deferred":false} -->

Current owner decision (2026-10-26): desktop is the complete release target. Phone/coordinator/APK requirements are preserved in [S09](../scopes/S09-future-mobile.md), separately deferred. All desktop batches are authorised sequentially.

## Work packet

**Outcome:** Scoped authenticated workload inspection and deliberate service/container control.

**User journey:** Select discovered workload → inspect/logs → review action → elevate if needed → observe actual resulting state.

**Scope:** [S05](../scopes/S05-observe-operate.md). **Feature prerequisites:** [F07](F07-monitoring.md), [F06](F06-ssh-elevation.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

No seed discovery applies beyond the shared scope. Link new findings when material.

## T022 — Implement protected workload inspection and logs

<!-- atlas-ticket: {"id":"T022","feature":"F11","status":"todo","depends_on":["T020","T013"],"evidence":null} -->

### Outcome and included work

Inspect discovered systemd services and supported containers with bounded on-demand logs.

### Exclusions

Automatically starting workloads, arbitrary shell logs or onboarding every possible runtime.

### Implementation and contracts

Use authenticated workload IDs and runtime/scope ownership, not arbitrary command strings. Fetch protected details/logs on demand, bound range/output, distinguish saved versus live state and clear results at lock/expiry.

**Ticket dependencies:** [T020](F07-monitoring.md), [T013](F06-ssh-elevation.md).

### Acceptance criteria

- [ ] System/user service and supported container identities resolve to their correct host/runtime scope.
- [ ] Log selection/output is bounded; permission-denied, stopped and unavailable workloads have explicit results.
- [ ] Protected details/logs are never persisted in overview records, telemetry or acceptance artifacts.

### Verification

Fixtures for systemd plus the supported runtime adapters, wrong-scope/unknown workload and bounded log paths; one inspection journey.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T022, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record unsupported runtime/account permissions as explicit limits requiring scope review, not an unrestricted fallback.

## T023 — Implement scoped workload control

<!-- atlas-ticket: {"id":"T023","feature":"F11","status":"todo","depends_on":["T022","T013","T014"],"evidence":null} -->

### Outcome and included work

Start, stop and restart an explicitly selected service/container after visible review and required elevation.

### Exclusions

Deployment/reconfiguration, bulk actions, global root/container grants or inventory-only grants treated as control permission.

### Implementation and contracts

Show host/workload/runtime/action, use allowlisted typed operations, fresh sudo where required and truthful post-action observation. Handle timeout/cancel/idempotency so reconnect or repeated taps do not duplicate control actions.

**Ticket dependencies:** [T022](F11-service-operations.md), [T013](F06-ssh-elevation.md), [T014](F06-ssh-elevation.md).

### Acceptance criteria

- [ ] Start/stop/restart work for the declared core systemd and container adapters on controlled workloads.
- [ ] Wrong scope/identity, expired session, denied sudo and duplicate requests cannot control another or unauthorised workload.
- [ ] UI reports actual observed completion/failure/unknown result, not request-sent success; no credentials/protected logs persist.

### Verification

Controlled workload state-transition tests and one reviewed native action journey with cancellation/timeout; no production restart as a test default.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T023, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop on missing operation permission or ambiguous target/outcome; report the exact state instead of repeating a potentially completed action.
