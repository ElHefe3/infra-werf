# S02 — Applications and map

<!-- atlas-scope: {"id":"S02","features":["F03","F04"],"depends_on":["S01"],"window":1,"deferred":false} -->

## Outcome

The approved map and workspaces rendered in real Windows/Android shells using shared React components and real domain persistence.

## Entry and sequence

S01 is complete. Use safe domain fixtures/copies; pairing is not available until S03.

Owner priority on 2026-10-25: desktop first. Deliver F03's desktop map/workspace, then T008 native lifecycle, then T009's separate compact phone composition/touch/native adapters. Phone requirements remain in this scope and core release; the acceptance gate below is unchanged. Thin build scaffolding is reused rather than regenerated.

**Scope dependencies:** [S01](S01-foundations.md). **Budget:** window 1.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F03](../features/F03-react-map.md) | T006, T007 | Desktop load → search/edit/connect → inspect workspace → reload. Compact-phone/touch composition remains in T009. |
| [F04](../features/F04-native-shells.md) | T008, T009, T032 | Native lifecycle and desktop operation guard, followed by compact phone/native adapters. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] Map editing/search/relationships and desktop positions persist without corrupting compact mobile arrangement.
- [ ] Approved 13-inch and compact phone compositions remain readable, accessible and overflow-free with real report shapes.
- [ ] Windows Tauri lifecycle/coordinator integration and Android native adapters are proven in the appropriate shells.
- [ ] Fixture transport and still-unimplemented operations are clearly distinguished from real authentication/control; no fake success.

## Verification and refinement

One shared map save/reload journey; targeted layout/focus checks at laptop and 320/390px widths; one native shell/adapters smoke per platform. Reserve real pairing/device release acceptance for S03/S06.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

Desktop progress: T006–T008 passed; [F03 proof](../evidence/F03-acceptance.md), [Windows lifecycle proof](../evidence/F04-desktop-acceptance.md). T009 and combined scope gates stay open. Remote phone/transfer close warnings must be verified as those features are enabled.

## Exclusions and stop boundary

Visual redesign, floating-window desktop, public endpoint, agent installation and release/device acceptance claims based only on a browser.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

S03; keep native build caches and shared component contracts stable.
