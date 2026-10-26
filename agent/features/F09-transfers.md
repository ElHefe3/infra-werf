# F09 — Visible recoverable SFTP transfers, including reviewed privileged placement, on Windows.

<!-- atlas-feature: {"id":"F09","scope":"S04","depends_on":["F08","F04"],"deferred":false} -->

Current owner decision (2026-10-26): desktop is the complete release target. Phone/coordinator/APK requirements are preserved in [S09](../scopes/S09-future-mobile.md), separately deferred. All desktop batches are authorised sequentially.

## Work packet

**Outcome:** Visible recoverable SFTP transfers, including reviewed privileged placement, on Windows.

**User journey:** Pick local file → choose remote destination → transfer/cancel/retry → download and compare → stage and elevate protected placement.

**Scope:** [S04](../scopes/S04-files-transfers.md). **Feature prerequisites:** [F08](F08-file-manager.md), [F04](F04-native-shells.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D003](../discovery/D003-android-prerequisites.md).

## T017 — Implement streamed transfer jobs

<!-- atlas-ticket: {"id":"T017","feature":"F09","status":"done","depends_on":["T015","T016","T008"],"evidence":"../evidence/F09-transfers-acceptance.md","scope_entry":{"S03":["T014"]}} -->

### Outcome and included work

Upload/download with native file selection, queue, progress, cancellation and recoverable retry.

### Exclusions

rsync, mirror deletion, background privileged transfer or unrestricted phone storage access.

### Implementation and contracts

Stream bytes rather than buffering whole files, supporting native Windows selection and bounded file streams; Android content URIs remain future S09. Jobs carry server/session/destination identity, structured state and bounded progress. Explicitly handle collisions/partial files; do not replay mutations on reconnect.

**Ticket dependencies:** [T015](F08-file-manager.md), [T016](F08-file-manager.md), [T008](F04-native-shells.md).

### Acceptance criteria

- [x] Upload/download round trips match checksums for empty, small and representative large files on Windows.
- [x] Progress/queue/cancel/failure/retry reflect actual byte/job state; interrupted reconnect does not silently duplicate writes.
- [x] Windows native selection authorises only the chosen file/destination; locked/expired sessions stop protected work and require reauthentication before retry. Android URI proof remains S09.

### Verification

Streaming/job-state tests and one native Windows round-trip/interruption journey with controlled files; inspect bounded memory behavior without a broad benchmark campaign. Android content-URI transfer proof remains future S09 work.

### Evidence and closure

Passed in [T017 desktop acceptance](../evidence/F09-transfers-acceptance.md) and [T018 protected-placement acceptance](../evidence/T018-acceptance.md): native picker/transfer and reviewed elevated placement journeys, fixture recovery proof, and bounded independent review. Startup discovery for force-quit staging remains noted in [D027](../discovery/D027-protected-stage-session-lifecycle.md).

### Stop conditions

Record unsupported provider/partial-transfer behavior instead of silently copying to a different destination or claiming resumability.

## T018 — Implement reviewed protected placement

<!-- atlas-ticket: {"id":"T018","feature":"F09","status":"done","depends_on":["T013","T016","T017"],"evidence":"../evidence/T018-acceptance.md"} -->

### Outcome and included work

Upload as the SSH user, then apply explicitly reviewed elevated destination/ownership changes.

### Exclusions

Permanent privileged SFTP server, sudo policy changes or protected live destinations without explicit authorisation.

### Implementation and contracts

Use a private staging path, show affected host/path/ownership/mode, request fresh sudo, and apply the narrow verified operation. Define rollback/cleanup for cancellation/failure and report retained safe staging information. Never run the entire transfer as root.

**Ticket dependencies:** [T013](F06-ssh-elevation.md), [T016](F08-file-manager.md), [T017](F09-transfers.md).

### Acceptance criteria

- [x] Unprivileged staging and protected application are distinct and require explicit reviewed elevation.
- [x] Wrong sudo/cancel/expired session or collision cannot silently overwrite the protected destination.
- [x] Success applies intended content/ownership/mode; interruption/cleanup behavior is proven and does not leak passwords or orphan unrestricted plaintext.

### Verification

Controlled protected destination fixture: success, wrong password, cancel and failed application; compare content/permissions and stage cleanup.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T018, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Passed in [T018 acceptance](../evidence/T018-acceptance.md): actual SSH/SFTP/sudo protected staging and placement, cancellation/failure/collision, cleanup and expiry fixture, Windows debug build, native elevation regression and bounded review. [D027](../discovery/D027-protected-stage-session-lifecycle.md) records the stage/Lock and expiry recovery contract. Absolute expiry closes the session even if a restricted stage remains; the UI exposes the exact location for ordinary-user recovery. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop if the destination operation cannot preserve data or isolate the elevated step; capture exact remaining stage/recovery state.
