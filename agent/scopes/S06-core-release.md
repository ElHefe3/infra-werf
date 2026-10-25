# S06 — Core release acceptance

<!-- atlas-scope: {"id":"S06","features":["F12"],"depends_on":["S04","S05"],"window":2,"deferred":false} -->

## Outcome

A packaged and owner-accepted native core release with data recovery and a reversible cutover.

## Entry and sequence

All included feature tickets have valid proof. Required owner/device access and signing/installation permissions are available or explicitly blocked.

T024 integrated/recovery proof, T025 release artifacts/install smoke, then T026 actual-phone acceptance and cutover. Reuse unchanged artifacts/evidence.

**Scope dependencies:** [S04](S04-files-transfers.md), [S05](S05-observe-operate.md). **Budget:** window 2.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F12](../features/F12-release-acceptance.md) | T024, T025, T026 | Migrate safe copy → install both apps → pair/control/transfer → restart/recover → owner accepts cutover. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] Shared data/auth/operation regressions and safe-copy migration/rollback pass.
- [ ] Pinned mise tasks produce Windows installer and signed Android APK with checksums/signature validation and controlled install results.
- [ ] Actual Windows/Android LAN-or-existing-VPN journeys cover full core controls, lifecycle/auth failures and recovery.
- [ ] Owner acceptance, remaining limitations, restart/availability and rollback instructions are recorded before deliberately retiring any fallback.

## Verification and refinement

One targeted integrated regression/recovery, one release build per target and one concise physical-device owner acceptance set. Do not rerun unchanged component suites for documentation cleanup.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

## Exclusions and stop boundary

Assuming owner acceptance, publishing stores, purchasing signing certificates, exposing public services or automatically entering deferred work.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

Core release complete only after T026 evidence. S07/S08 require a separate explicit owner activation.
