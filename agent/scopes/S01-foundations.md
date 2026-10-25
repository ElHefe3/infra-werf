# S01 — Foundations

<!-- atlas-scope: {"id":"S01","features":["F01","F02"],"depends_on":[],"window":1,"deferred":false} -->

## Outcome

A pinned latest-stable mise environment, early native build proof and a safe Rust persistence/import foundation.

## Entry and sequence

Read the recovered stack and D001–D004. Application implementation must be authorised; do not confuse this tracker-creation task with T001 execution.

F01 proves effective versions and thin Windows/Android builds before domain/persistence work. Then F02 establishes contracts, data parity and inventory classification.

**Scope dependencies:** None. **Budget:** window 1.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F01](../features/F01-build-environment.md) | T001, T002 | Fresh project environment → effective pinned versions → thin native Windows and Android builds. |
| [F02](../features/F02-domain-persistence.md) | T003, T004, T005 | Safe legacy fixture/copy → Rust read/write/import → restart/parity → rollback. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] Mise selects current stable pinned tools and a Windows MSVC Rust host; prerequisite gaps are explicit.
- [ ] Thin Windows x64 and Android ARM64 builds are demonstrated, rather than inferred from installed folders.
- [ ] Rust persistence/import preserve IDs, layout, relationships, source timestamps and owner edits on controlled copies; invalid migration/import is recoverable.
- [ ] Inventory retention classification and shared contracts respect protected-information boundaries; F02 risk review is recorded when available.

## Verification and refinement

Reuse the reference's meaningful persistence/import cases; add only migration/contract/allowlist failures. One thin build per platform, one parity/rollback fixture run, one focused foundation review.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

## Exclusions and stop boundary

No live database cutover, production SSH/host changes, full UI rewrite or global tool-default replacement.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

S02. Reassess budget immediately if the first native builds expose a setup blocker.
