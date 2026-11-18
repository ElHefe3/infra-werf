# D065 — The owner requests an interface redesign

Date: 2026-11-18. Related: S14 / F27 T059–T062, [D056](D056-categorised-detachable-main-menu.md), [D064](D064-menu-pop-outs-disabled.md).

## Owner decision

On 2026-11-18 the owner asked to change the visual style without a plugin, to "see how this would be as liquid glass". In the same session they also asked for:

- a more transparent window with a nearly clear background;
- the branding taglines removed;
- a dark theme;
- larger, readable text that still treats screen space as precious;
- a design where the map is the main surface and everything else supports it;
- the menu pop-outs disabled ([D064](D064-menu-pop-outs-disabled.md)).

## What this means

- `SCOPE.md` excludes a "visual redesign", and D056 kept that exclusion for everything except the menu. This decision narrowly widens it: the look, theme and layout of the main window may change. Behaviour, data and access boundaries do not.
- Plugins still cannot change InfraWerf's look. Plugin theming would need a new extension point and is not planned.
- The classic look stays available from the top bar.

## Still open

- Whether acrylic window resizing is smooth enough on the owner's PC, or the Mica effect is needed instead.
