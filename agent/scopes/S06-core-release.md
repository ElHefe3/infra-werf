# S06 — Core release acceptance

<!-- atlas-scope: {"id":"S06","features":["F12"],"depends_on":["S04","S05"],"window":2,"deferred":false,"evidence":"../evidence/S06-acceptance.md"} -->

Mobile/APK/pairing/physical-phone requirements are retained in [future S09](S09-future-mobile.md).

## Outcome

A packaged and owner-accepted native core release with data recovery and a reversible cutover.

## Entry and sequence

All included desktop feature tickets have valid proof. Required owner access and Windows installation permissions are available or explicitly blocked. Mobile signing, phone installation and pairing remain future S09 work.

T024 integrated/recovery proof, T025 release artifacts/install smoke, then T026 desktop acceptance and cutover. Reuse unchanged artifacts/evidence.

**Scope dependencies:** [S04](S04-files-transfers.md), [S05](S05-observe-operate.md). **Budget:** window 2.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F12](../features/F12-release-acceptance.md) | T024, T025, T026 | Migrate safe copy → install Windows app → authenticate/control/transfer → restart/recover → owner accepts cutover. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [x] Shared data/auth/operation regressions and safe-copy migration/rollback pass.
- [x] Pinned mise tasks produce Windows installer with checksums/signature validation and controlled install results.
- [x] Actual Windows desktop journeys cover the installed app's saved data, restart, strict-trust SSH and reused core control/lifecycle/auth/recovery proof.
- [x] Owner acceptance, remaining limitations, restart/availability and rollback instructions are recorded; both data fallbacks remain available.

## Verification and refinement

One targeted integrated regression/recovery, one Windows release build and one concise owner acceptance set on the actual Windows desktop. Do not rerun unchanged component suites for documentation cleanup.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

## Exclusions and stop boundary

Assuming owner acceptance, publishing stores, purchasing signing certificates, exposing public services or automatically entering deferred work.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

Core release complete: see [S06 acceptance](../evidence/S06-acceptance.md) and [T026 acceptance](../evidence/T026-acceptance.md). The installed Windows desktop is the primary app; the Python reference and native recovery snapshot remain fallbacks. S07/S08 require separate explicit owner activation.
