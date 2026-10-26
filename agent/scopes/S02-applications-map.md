# S02 — Applications and map

<!-- atlas-scope: {"id":"S02","features":["F03","F04"],"depends_on":["S01"],"window":1,"deferred":false,"evidence":"../evidence/S02-desktop-acceptance.md"} -->

## Outcome

The approved map and workspaces rendered in the Windows desktop shell using shared React components and real domain persistence. Compact mobile composition and Android adapters remain in future S09.

## Entry and sequence

S01 is complete. Use safe domain fixtures/copies. Phone pairing and mobile access are deferred to S09.

Owner priority on 2026-10-25: desktop first. Deliver F03's desktop map/workspace, then T008 native lifecycle, then T009's separate compact phone composition/touch/native adapters. The owner moved mobile to S09 on 2026-10-26; desktop gate evidence below is reused without claiming phone acceptance. Thin build scaffolding is reused rather than regenerated.

**Scope dependencies:** [S01](S01-foundations.md). **Budget:** window 1.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F03](../features/F03-react-map.md) | T006, T007 | Desktop load → search/edit/connect → inspect workspace → reload. Compact-phone/touch composition remains in T009. |
| [F04](../features/F04-native-shells.md) | T008, T032, T033 | Windows native lifecycle, managed-operation guard and strict-trust SSH verification. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [x] Desktop map editing/search/directional relationships and layout persist through native restart.
- [x] Approved laptop compositions remain readable and overflow-free with the saved inventory.
- [x] Windows native lifecycle, single instance, operation admission and actual Rust IPC are proven.
- [x] Saved/fixture/unavailable states remain truthful; authentication is never inferred from saved inventory.

## Verification and refinement

One shared map save/reload journey; targeted layout/focus checks at approved laptop widths; one actual Windows native shell journey. Phone-size composition, Android adapters and pairing are tested only when S09 is activated.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

Desktop progress: T006–T008, T032 and T033 passed; [map proof](../evidence/F03-acceptance.md), [Windows proof](../evidence/F04-desktop-acceptance.md), [SSH verification proof](../evidence/T033-acceptance.md). Desktop S02 is passed; T009 is deferred in S09. Transfer close-warning proof is recorded with T017; remote phone lifecycle remains S09.

## Exclusions and stop boundary

Visual redesign, floating-window desktop, public endpoint, agent installation and release/device acceptance claims based only on a browser.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

S03; keep native build caches and shared component contracts stable.
