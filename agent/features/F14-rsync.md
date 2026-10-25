# F14 — Deferred efficient directory synchronisation with reviewed changes and no implicit destination deletion.

<!-- atlas-feature: {"id":"F14","scope":"S08","depends_on":["F12","F09"],"deferred":true} -->

## Work packet

**Outcome:** Deferred efficient directory synchronisation with reviewed changes and no implicit destination deletion.

**User journey:** Verify Windows runtime → dry-run selected directories → copy/update → interrupt/retry → explicitly review optional deletion.

**Scope:** [S08](../scopes/S08-synchronisation.md). **Feature prerequisites:** [F12](F12-release-acceptance.md), [F09](F09-transfers.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

No seed discovery applies beyond the shared scope. Link new findings when material.

## T030 — Select and verify Windows rsync runtime

<!-- atlas-ticket: {"id":"T030","feature":"F14","status":"deferred","depends_on":["T026"],"evidence":null} -->

### Outcome and included work

Choose a current supported Windows rsync execution path compatible with the SSH identity and native application.

### Exclusions

Automatic WSL/system installation, switching transfer defaults or substituting SSH credentials.

### Implementation and contracts

Compare practical installed/runtime options, owner installation implications, path/Unicode behavior, licensing and command/agent interoperability. Record one decision and pin it through the project setup where supported.

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Chosen runtime/version and installation implications are recorded, with owner approval where required.
- [ ] Windows/local-to-remote path conversion, spaces/Unicode and existing SSH identity behavior pass a controlled fixture.
- [ ] Packaging/discovery/missing-runtime diagnostics avoid silently using a different binary.

### Verification

One narrow runtime interoperability probe; no application rewrite or broad tool comparison campaign.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T030, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Remain deferred until S08 activation; stop if installation/policy choice lacks approval.

## T031 — Implement previewed rsync jobs and recovery

<!-- atlas-ticket: {"id":"T031","feature":"F14","status":"deferred","depends_on":["T030","T017","T018"],"evidence":null} -->

### Outcome and included work

Add optional directory copy/update jobs with preview, progress, cancellation/recovery and separately explicit deletion mode.

### Exclusions

Automatic mirroring, scheduled destructive jobs or bypassing SSH/sudo sessions.

### Implementation and contracts

Integrate the existing transfer queue/session model. Default to non-deleting copy/update; show selected endpoints and dry-run changes. Any destination deletion needs a distinct reviewed confirmation and validated options, not a stored hidden flag.

**Ticket dependencies:** [T030](F14-rsync.md), [T017](F09-transfers.md), [T018](F09-transfers.md).

### Acceptance criteria

- [ ] Default synchronisation preserves destination-only files; preview matches actual copy/update for controlled trees.
- [ ] Delete mode requires explicit reviewed scope/confirmation and cancellation changes nothing.
- [ ] Interrupt/retry/expiry preserve job truth and authentication boundaries; protected application uses the established staging/elevation path.

### Verification

Controlled trees/checksums with destination-only files, unusual names, deletion preview/cancel and interrupted retry.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T031, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop on unclear delete scope or preview/actual mismatch; never retry a destructive result blindly.
