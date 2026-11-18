# S14 — Interface look and layout

<!-- atlas-scope: {"id":"S14","features":["F27"],"depends_on":["S06","S12"],"window":2,"deferred":false} -->

## Outcome

A desktop interface built around the map: a liquid glass look over a see-through window, a dark theme, readable text, and supporting controls that take as little room as possible.

## Entry and sequence

The owner requested this on 2026-11-18 ([D065](../discovery/D065-owner-requests-interface-redesign.md)). It narrowly widens the `SCOPE.md` exclusion of a visual redesign. The classic look stays available and unchanged in behaviour.

T059 (look) comes first; T060 (dark theme) and T061 (layout) build on it. T062 (menu pop-outs disabled) is independent.

**Scope dependencies:** [S06](S06-core-release.md), [S12](S12-extensibility.md). **Budget:** owner-authorised extension; no fixed window.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F27](../features/F27-interface-look.md) | T059–T065 | Open InfraWerf → the map fills the window → switch look and theme from the top bar → hide and show the menu and inspector → messages appear and fade without taking space. |

Ticket status is canonical in the feature packets; scope progress is derived.

## Acceptance gate

- [ ] The owner accepts the glass and classic looks, in light and dark, on the installed build against a real server.
- [ ] The map keeps most of the window at the default size; supporting panels can be hidden and brought back in the same way.
- [ ] No behaviour, data or access boundary changes beyond what F27 lists.

## Verification and refinement

Browser previews with made-up data during design; the desktop journey (`scripts/verify_desktop.mjs`) for layout and menu behaviour; owner review of the installed build on the real workspace.

## Exclusions and stop boundary

No plugin theming (that would be a new extension point), no change to what data is shown or fetched, no floating content windows.

## Exit

Owner acceptance of the installed build.
