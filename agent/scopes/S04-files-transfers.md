# S04 — Files and transfers

<!-- atlas-scope: {"id":"S04","features":["F08","F09"],"depends_on":["S03"],"window":2,"deferred":false} -->

## Outcome

An authenticated file-manager/transfer journey on Windows and native Android, including deliberate protected placement.

## Entry and sequence

S03 is complete. Use controlled local/remote files, scoped Android providers and explicitly approved protected test destinations.

F08 browsing/stat precede file mutations; F09 reuses that session/path boundary for streaming jobs and protected staging.

**Scope dependencies:** [S03](S03-secure-connections.md). **Budget:** window 2.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F08](../features/F08-file-manager.md) | T015, T016 | Open Files with SSH → navigate/select/stat → create/rename/move → confirm a targeted delete → lock and clear. |
| [F09](../features/F09-transfers.md) | T017, T018 | Pick local file → choose remote destination → transfer/cancel/retry → download and compare → stage and elevate protected placement. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] Navigation/stat/permissions and deliberate mkdir/rename/move/delete handle unusual names, symlinks, collisions and denial.
- [ ] Uploads/downloads stream through native Windows/Android I/O; round-trip checksums match and progress reflects actual work.
- [ ] Cancel/interruption/retry/expiry are recoverable without silent duplicate writes or hidden destructive defaults.
- [ ] Protected placement stages unprivileged bytes and applies only reviewed narrow elevation, with content/ownership and cleanup proof.

## Verification and refinement

One controlled file round trip/edit/interruption journey per native client plus small edge-case/path/job tests. Reuse auth evidence unless those boundaries change.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

## Exclusions and stop boundary

rsync, mirror deletion, broad phone-storage permissions, whole transfers as root and production file deletion as a default test.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

S05, or return to any independently authorised operational feature already in progress.
