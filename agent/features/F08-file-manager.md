# F08 — An authenticated remote filesystem workspace with safe navigation and deliberate mutations.

<!-- atlas-feature: {"id":"F08","scope":"S04","depends_on":["F06","F03"],"deferred":false} -->

Current owner decision (2026-10-26): desktop is the complete release target. Phone/coordinator/APK requirements are preserved in [S09](../scopes/S09-future-mobile.md), separately deferred. All desktop batches are authorised sequentially.

## Work packet

**Outcome:** An authenticated remote filesystem workspace with safe navigation and deliberate mutations.

**User journey:** Open Files with SSH → navigate/select/stat → create/rename/move → confirm a targeted delete → lock and clear.

**Scope:** [S04](../scopes/S04-files-transfers.md). **Feature prerequisites:** [F06](F06-ssh-elevation.md), [F03](F03-react-map.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

Relevant discoveries: [D017 desktop feature entry](../discovery/D017-desktop-feature-entry.md), [D018 SFTP lifecycle/page recovery](../discovery/D018-sftp-process-and-page-lifecycle.md). Reuse the protected session and opaque-resource boundary for later mutations/transfers.

## T015 — Implement authenticated SFTP browsing

<!-- atlas-ticket: {"id":"T015","feature":"F08","status":"done","depends_on":["T014","T033","T007"],"scope_entry":{"S03":["T014"]},"evidence":"../evidence/F06-F08-desktop-acceptance.md"} -->

### Outcome and included work

Browse remote directories/metadata through actual SFTP on desktop with shared contracts. Compact-phone navigation and paired result-clearing proof are retained in T009/T010; they are future S09 requirements.

### Exclusions

Recursive content indexing, file transfer engines, automatic root browsing or pre-collecting directory trees.

### Implementation and contracts

Support breadcrumbs/back, selection, stat/permissions/ownership and bounded/paged listings. Preserve remote path encoding and symlink identity; do not assume Windows path rules. Directory contents are protected, ephemeral results.

**Ticket dependencies:** [T014](F06-ssh-elevation.md), [T033](F04-native-shells.md), [T007](F03-react-map.md). Read-only browsing requires protected authentication/lifecycle, not sudo. T013 remains mandatory before elevation; T010 retains paired Files proof.

### Acceptance criteria

- [x] Root/home/deep/large directories, Unicode/space names, symlinks and permission-denied paths render correctly.
- [x] Navigation uses authenticated server/session context; locked/expired sessions cannot retain visible cached listings.
- [x] Desktop selection/breadcrumbs/paging are usable with bounded memory/output; phone selection/breadcrumb/clearing journeys remain explicitly in T009/T010.

### Verification

Controlled real SFTP fixture with large/unusual/denied directories and an actual native Windows navigation/stat/clearing journey. Compact-phone/paired journeys remain T009/T010, with physical-phone acceptance in S09.

### Evidence and closure

Passed the desktop/core slice in [T014/T015 acceptance](../evidence/F06-F08-desktop-acceptance.md), with actual SFTP/native and reviewed lifecycle evidence. F08/S04 still require mutations, transfers and both-client proof. Resume from [the checkpoint](../evidence/CHECKPOINT.md).

### Stop conditions

Stop a path/encoding ambiguity before normalising away meaningful remote names or crossing access boundaries.

## T016 — Implement deliberate file mutations

<!-- atlas-ticket: {"id":"T016","feature":"F08","status":"done","depends_on":["T015"],"scope_entry":{"S03":["T014"]},"evidence":"../evidence/F08-mutations-acceptance.md"} -->

### Outcome and included work

Create directories, rename/move entries and explicitly confirm targeted deletion within an authenticated session.

Desktop-first slice authorised by the owner's “next” request. Reuse T014/T015; compact-phone mutation/confirmation/error/clearing proof remains T009/T010 in future S09. No overwrite or recursive delete is enabled in this slice. Proof: real SFTP fixture plus one native create/rename/move/delete/cancel journey, with stale/collision/denial/uncertain-result negatives and independent review. Stop after this bounded journey is recorded; do not start transfers automatically.

### Exclusions

Mirror deletion, automatic cleanup of user data or broad recursive operation defaults.

### Implementation and contracts

Present exact host/source/destination, collision and overwrite choices. Do not follow symlinks unexpectedly or silently recurse/delete. Validate operation arguments centrally and report partial/permission errors without corrupting view state.

**Ticket dependencies:** [T015](F08-file-manager.md).

### Acceptance criteria

- [x] Desktop mkdir/rename/move and collision/permission failure paths work for unusual filenames; compact-phone equivalents remain explicitly T009/T010.
- [x] Delete identifies the exact affected target and requires explicit confirmation; cancellation changes nothing and recursive deletion is separately explicit.
- [x] No expired/wrong-host session can mutate; views reconcile actual outcomes without optimistic false success.

### Verification

Actual-system SFTP fixture tests plus native Windows create/rename/move/delete/cancel/failure journey. Paired compact-phone equivalents remain T009/T010; recursive deletion is unavailable.

### Evidence and closure

Passed the desktop slice in [T016 acceptance](../evidence/F08-mutations-acceptance.md), including actual SFTP/native proof and resolved independent review. S04 retains desktop transfer/elevation acceptance; phone is future S09. See [D019](../discovery/D019-sftp-mutation-outcomes.md) and [the checkpoint](../evidence/CHECKPOINT.md).

### Stop conditions

Pause when operation atomicity, path target or data-preservation intent is unclear; do not infer permission to replace/delete.
