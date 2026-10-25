# F02 — One typed Rust domain and verified SQLite parity without losing existing map data.

<!-- atlas-feature: {"id":"F02","scope":"S01","depends_on":["F01"],"deferred":false} -->

## Work packet

**Outcome:** One typed Rust domain and verified SQLite parity without losing existing map data.

**User journey:** Safe legacy fixture/copy → Rust read/write/import → restart/parity → rollback.

**Scope:** [S01](../scopes/S01-foundations.md). **Feature prerequisites:** [F01](F01-build-environment.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D004](../discovery/D004-migration-baseline.md).

## T003 — Define shared domain and operation contracts

<!-- atlas-ticket: {"id":"T003","feature":"F02","status":"ready","depends_on":["T002"],"evidence":null} -->

### Outcome and included work

Establish stable typed contracts used by React clients, native commands and the coordinator.

### Exclusions

Implementing every operation, authentication itself or speculative plugin APIs.

### Implementation and contracts

Represent servers/edges, observation source/time/freshness, workload identity, structured errors, authenticated session/operation handles and progress events once. Keep UI/native transports thin; distinguish absent metrics from zero and host reachability from service health.

**Ticket dependencies:** [T002](F01-build-environment.md).

### Acceptance criteria

- [ ] Both clients consume one contract definition/version with stable IDs and no duplicated business rules.
- [ ] Errors distinguish invalid input, unauthenticated/denied, host-trust failure, expired session and I/O failure.
- [ ] Protected result types are separated from persistable approved baseline records; private keys/passwords are not serialisable domain data.

### Verification

Focused serialization/validation tests for representative DTOs and error mapping; type-check both adapters.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T003, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record a contract decision if requirements conflict with access boundaries or existing data preservation; do not invent broader privileges.

## T004 — Port SQLite and preserve map data

<!-- atlas-ticket: {"id":"T004","feature":"F02","status":"todo","depends_on":["T003"],"evidence":null} -->

### Outcome and included work

Implement Rust persistence with verified read/write/restart parity against the existing SQLite model.

### Exclusions

Cutting over the live app, dropping unclassified data, broad schema redesign or application decommissioning.

### Implementation and contracts

Use synthetic fixtures and a SQLite backup/copy, never the only live database. Preserve server IDs, owner names/hostnames, positions, edges and supported history. Version migration; keep unclassified imported data inaccessible pending T005. Keep Python reference available.

**Ticket dependencies:** [T003](F02-domain-persistence.md).

### Acceptance criteria

- [ ] CRUD, duplicate validation, coordinate rules and directional relationship semantics have parity across restart.
- [ ] Migration preserves existing IDs/layout/relationships and approved records without destructive replacement.
- [ ] Failure leaves the original/copy recoverable; rollback is exercised and concurrent Python/Rust writing to one database is prevented.

### Verification

Port meaningful persistence/migration cases from the ten-test reference suite; compare a safe copy once, including failed migration and rollback. Obtain bounded review when available.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T004, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop before any irreversible data transformation or unexplained parity mismatch; preserve the failing copy and report the difference.

## T005 — Classify and import approved inventory

<!-- atlas-ticket: {"id":"T005","feature":"F02","status":"todo","depends_on":["T004"],"evidence":null} -->

### Outcome and included work

Port bounded, atomic inventory import while enforcing the baseline/protected-information boundary.

### Exclusions

Live SSH collection, new inventory grants or automatic reclassification of all existing protected metadata.

### Implementation and contracts

Audit each retained field against the original access model. Permit approved baseline host/workload metadata only; never extend import to configs/logs/secrets. Preserve observation times and owner edits, ignore older/equal reports, reject invalid batches atomically.

**Ticket dependencies:** [T004](F02-domain-persistence.md).

### Acceptance criteria

- [ ] A recorded field classification explains what is retained, excluded or requires on-demand authenticated collection.
- [ ] Valid newer imports preserve stable identity/layout/owner edits; duplicate/older and invalid batches behave correctly.
- [ ] Views label reports as saved/stale and protected/raw fields never enter persistent overview state.

### Verification

Focused allowlist, timestamp/order, size/type, atomicity and preservation tests with malicious/raw fields in fixtures.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T005, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Pause affected fields if baseline approval is unclear; isolate them rather than silently exposing or destroying them.
