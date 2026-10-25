# S08 — Deferred directory synchronisation

<!-- atlas-scope: {"id":"S08","features":["F14"],"depends_on":["S06"],"window":null,"deferred":true} -->

## Outcome

Optional rsync directory jobs with verified Windows runtime, preview and explicit deletion control.

## Entry and sequence

Owner explicitly activates S08 after core release and accepts any new Windows runtime/installation implications.

Verify one supported runtime/SSH/path integration, then integrate previewed jobs into the existing queue/session/elevation model.

**Scope dependencies:** [S06](S06-core-release.md). **Budget:** deferred; outside the two-window core target.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F14](../features/F14-rsync.md) | T030, T031 | Verify Windows runtime → dry-run selected directories → copy/update → interrupt/retry → explicitly review optional deletion. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] Pinned runtime interoperates with selected Windows paths/Unicode and existing SSH identity.
- [ ] Copy/update is non-deleting by default; delete mode requires a distinct reviewed confirmation.
- [ ] Dry-run/actual outcomes, interruption/retry and protected application preserve existing data/auth boundaries.

## Verification and refinement

One narrow runtime probe and controlled directory/checksum/preview/cancel/retry journey; no broad runtime comparison loop.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

## Exclusions and stop boundary

Core budget, automatic WSL/runtime installation, hidden mirror flags or scheduled destructive sync.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

Optional feature acceptance; no automatic expansion into general backup/scheduler products.
