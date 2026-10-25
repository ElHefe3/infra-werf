# F08 — An authenticated remote filesystem workspace with safe navigation and deliberate mutations.

<!-- atlas-feature: {"id":"F08","scope":"S04","depends_on":["F06","F03"],"deferred":false} -->

## Work packet

**Outcome:** An authenticated remote filesystem workspace with safe navigation and deliberate mutations.

**User journey:** Open Files with SSH → navigate/select/stat → create/rename/move → confirm a targeted delete → lock and clear.

**Scope:** [S04](../scopes/S04-files-transfers.md). **Feature prerequisites:** [F06](F06-ssh-elevation.md), [F03](F03-react-map.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

No seed discovery applies beyond the shared scope. Link new findings when material.

## T015 — Implement authenticated SFTP browsing

<!-- atlas-ticket: {"id":"T015","feature":"F08","status":"todo","depends_on":["T013","T014","T007"],"evidence":null} -->

### Outcome and included work

Browse remote directories and metadata through SFTP on both clients.

### Exclusions

Recursive content indexing, file transfer engines, automatic root browsing or pre-collecting directory trees.

### Implementation and contracts

Support breadcrumbs/back, selection, stat/permissions/ownership and bounded/paged listings. Preserve remote path encoding and symlink identity; do not assume Windows path rules. Directory contents are protected, ephemeral results.

**Ticket dependencies:** [T013](F06-ssh-elevation.md), [T014](F06-ssh-elevation.md), [T007](F03-react-map.md).

### Acceptance criteria

- [ ] Root/home/deep/large directories, Unicode/space names, symlinks and permission-denied paths render correctly.
- [ ] Navigation uses authenticated server/session context; locked/expired sessions cannot retain visible cached listings.
- [ ] Desktop and phone selection/breadcrumb behavior is usable with bounded listing memory/output.

### Verification

Controlled SFTP fixture with large/unusual/denied directories and one navigation/stat journey on each client.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T015, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop a path/encoding ambiguity before normalising away meaningful remote names or crossing access boundaries.

## T016 — Implement deliberate file mutations

<!-- atlas-ticket: {"id":"T016","feature":"F08","status":"todo","depends_on":["T015"],"evidence":null} -->

### Outcome and included work

Create directories, rename/move entries and explicitly confirm targeted deletion within an authenticated session.

### Exclusions

Mirror deletion, automatic cleanup of user data or broad recursive operation defaults.

### Implementation and contracts

Present exact host/source/destination, collision and overwrite choices. Do not follow symlinks unexpectedly or silently recurse/delete. Validate operation arguments centrally and report partial/permission errors without corrupting view state.

**Ticket dependencies:** [T015](F08-file-manager.md).

### Acceptance criteria

- [ ] mkdir/rename/move and collision/permission failure paths work for unusual filenames on both clients.
- [ ] Delete identifies the exact affected target and requires explicit confirmation; cancellation changes nothing and recursive deletion is separately explicit.
- [ ] No expired/wrong-host session can mutate; views reconcile actual outcomes without optimistic false success.

### Verification

Small fixture operation tests including cancellation, collisions, symlinks and adversarial names; one controlled edit/delete journey.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T016, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Pause when operation atomicity, path target or data-preservation intent is unclear; do not infer permission to replace/delete.
