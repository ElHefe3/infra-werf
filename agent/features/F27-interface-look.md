# F27 — Interface look and canvas-first layout

<!-- atlas-feature: {"id":"F27","scope":"S14","depends_on":["F12","F18"],"deferred":false} -->

## Work packet

**Outcome:** The interface reads clearly, gives the map the space, and can be shown as liquid glass or classic, light or dark.

**User journey:** Open InfraWerf → the map fills the window → use the top-bar buttons to switch look (glass/classic) and theme (light/dark) → hide the menu or the inspector with the round button inside it and bring it back with the floating one → a confirmation appears over the content and fades.

**Scope:** [S14](../scopes/S14-interface-look.md). **Feature prerequisites:** [F12](F12-release-acceptance.md), [F18](F18-extension-points.md).

Context: requested by the owner on 2026-11-18 ([D065](../discovery/D065-owner-requests-interface-redesign.md)). Implemented in the working tree on 2026-11-18; not yet accepted.

## T059 — Liquid glass look and see-through window

<!-- atlas-ticket: {"id":"T059","feature":"F27","status":"in_progress","depends_on":["T026"],"evidence":null} -->

### Outcome and included work

A liquid glass look (`ui/liquid-glass.css`) layered over the classic stylesheet and active only while `<html data-look="glass">`. Content panels are frosted glass; the page background is almost clear, and the main window is transparent with the Windows acrylic effect (`src-tauri/tauri.conf.json`) so the desktop shows through, blurred. A top-bar button switches look; the choice is a per-PC preference in webview storage (`ui/look.ts`). Glass is the default.

### Exclusions

Plugin-controlled theming; changing remote-application windows.

### Implementation and contracts

`<html data-look>` selects the look; glass rules carry the attribute so removing it restores classic. The window uses `transparent: true` with `windowEffects: acrylic` and a near-zero tint; the classic page paints an opaque background over it. The preference is stored only in webview storage.

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Glass shows the desktop through the window background while panels stay readable.
- [ ] The classic look is unchanged apart from F27's layout changes.
- [ ] The look switch names the action it performs and the choice survives a restart.
- [ ] With Windows' reduced-transparency setting, the main panels become solid.

### Verification

`ui/look.test.ts`; browser previews; owner review on the installed build. Watch for slow window resizing with acrylic (Mica is the fallback).

### Evidence and closure

Not yet accepted. Close with owner acceptance on the installed build against a real server, recorded in `agent/evidence/`.

### Stop conditions

Stop if a change would alter what data is fetched or shown, or an access boundary.

## T060 — Dark theme for both looks

<!-- atlas-ticket: {"id":"T060","feature":"F27","status":"in_progress","depends_on":["T026"],"evidence":null} -->

### Outcome and included work

A dark theme (`ui/dark.css`, `<html data-theme="dark">`) for glass (smoked glass) and classic (solid dark). It follows Windows' app mode until the owner chooses, then remembers the choice. Map relationship labels use theme colours.

### Exclusions

Per-server or per-plugin themes.

### Implementation and contracts

Dark rules carry one more attribute than the light rules they replace, so they win without `!important`. Map label colours come from CSS variables passed to React Flow. The default reads `prefers-color-scheme`. It builds on T059's stylesheets; it depends on T026 in metadata because T059 is not yet closed.

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] All text, status pills, buttons, inputs, dialogs and map labels are legible in dark, in both looks.
- [ ] The theme switch names its action; the default follows Windows until a choice is saved.

### Verification

`ui/look.test.ts`; browser previews of both looks; owner review.

### Evidence and closure

Not yet accepted. Close with owner acceptance on the installed build against a real server, recorded in `agent/evidence/`.

### Stop conditions

Stop if a change would alter what data is fetched or shown, or an access boundary.

## T061 — Canvas-first layout and readable text

<!-- atlas-ticket: {"id":"T061","feature":"F27","status":"in_progress","depends_on":["T026"],"evidence":null} -->

### Outcome and included work

- Branding taglines removed (intro heading, map caption, access note, footer).
- Summary counts moved into the top bar; Add server moved there too.
- The access note, the bottom footer and the map footer removed; the legend sits in the map corner; report import appears in the map toolbar only when available.
- Status messages float over the content; confirmations fade after 3 s, errors stay until dismissed.
- The inspector has no title row; it hides with a round button inside it and returns with a floating one, matching the main menu toggle.
- The local avatar removed from the menu.
- Transfers in the main panel uses the same inset as Activity, Settings and Servers; dark glass panels are more opaque so text stays readable over light desktops.
- Minimum text size raised from 8–9 px to 11 px; body text 14 px.

### Exclusions

Changing what the inspector or map shows.

### Implementation and contracts

Layout changes live in `ui/main.tsx` and the canvas-first section of `ui/style.css`, with glass and dark counterparts. Removed elements are not hidden but taken out of the page. The inspector toggle reuses the menu toggle's size and shape (`.panel-toggle`). It builds on T059's stylesheets; it depends on T026 in metadata because T059 is not yet closed.

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] At 1280×820 the map panel uses at least the full height below the top bar.
- [ ] Menu and inspector hide and return the same way.
- [ ] No text in the main window is smaller than 11 px.
- [ ] The desktop journey passes with the new layout.

### Verification

`scripts/verify_desktop.mjs` (now measures the map panel instead of the removed footer); owner review.

### Evidence and closure

Not yet accepted. Close with owner acceptance on the installed build against a real server, recorded in `agent/evidence/`.

### Stop conditions

Stop if a change would alter what data is fetched or shown, or an access boundary.

## T062 — Disable detachable menu categories

<!-- atlas-ticket: {"id":"T062","feature":"F27","status":"in_progress","depends_on":["T053"],"evidence":null} -->

### Outcome and included work

Menu pop-outs from T053 are switched off (`MENU_POPOUT_ENABLED = false` in `ui/menu.tsx`): no detach buttons and no floating panels. Saved floating panels are ignored, not deleted. The model code and its tests stay. Reason and future direction (configuration, not navigation) in [D064](../discovery/D064-menu-pop-outs-disabled.md).

### Exclusions

Removing the pop-out model code or the saved layout.

### Implementation and contracts

One constant gates the detach buttons and the floating panels; the Settings text describes only the remaining menu behaviour.

**Ticket dependencies:** [T053](F18-extension-points.md).

### Acceptance criteria

- [ ] No detach control or floating menu appears, including with a previously saved layout.
- [ ] Hiding the menu, Ctrl+B, collapsing categories and resetting the layout still work.

### Verification

`scripts/verify_desktop.mjs` checks the absence of detach controls and the remaining menu behaviour.

### Evidence and closure

Not yet accepted. Close with owner acceptance on the installed build against a real server, recorded in `agent/evidence/`.

### Stop conditions

Stop if a change would alter what data is fetched or shown, or an access boundary.

## T063 — Separate relationship lines on the map

<!-- atlas-ticket: {"id":"T063","feature":"F27","status":"in_progress","depends_on":["T026"],"evidence":null} -->

### Outcome and included work

Each relationship gets its own connection point on the side of each card that faces the other server, spread along that side in the order of the far ends (`routeEdges` in `ui/map-model.ts`). Each label is then placed along its own line where it covers no other label and no card, trying the middle first (`placeLabels`). The original left and right handles remain for drawing new relationships.

### Exclusions

Routing lines around cards that sit between two connected servers (a line can still pass behind a card; its label is kept clear of it).

### Implementation and contracts

Routes are computed from saved positions and recomputed after a move is saved; React Flow is told when a card's connection points change. Routing handles cannot start a new relationship.

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Several relationships from one server leave from separate points, and no two labels overlap or sit on a card.
- [ ] A relationship to a server below or above uses the bottom or top of the card.
- [ ] Drawing a new relationship still works.

### Verification

`ui/map-model.test.ts`; browser preview with a busy sample map; owner review on the real map.

### Evidence and closure

Not yet accepted. Close with owner acceptance on the installed build against a real server, recorded in `agent/evidence/`.

### Stop conditions

Stop if routing changes saved positions or relationships.

## T064 — Apps tab tiles, cached discovery and app icons

<!-- atlas-ticket: {"id":"T064","feature":"F27","status":"in_progress","depends_on":["T026"],"evidence":null} -->

### Outcome and included work

- The Apps tab shows discovered web applications as a tile grid with a short name and icon; address, process, page title, version (from the page's `generator` tag) and status sit behind an ⓘ button, which needs the PIN when one is set (T065). Other listening ports are listed under a disclosure, and the manual port entry stays available.
- The last discovery for each server is kept on this PC and shown at once; a fresh discovery runs when the tab opens and replaces it. Discovery holds the live-contact lock only while its session opens, so a cached app opens without waiting for the listing to finish.
- Discovery also reads each app's favicon (its `<link rel=icon>` path or `/favicon.ico`), at most 6 KiB each and about 40 KB per run. Apps without a usable icon get a coloured letter tile.
- Native dropdowns are legible in dark mode.
- Tiles keep one size; a cached address-specific app waiting for discovery shows a small spinner instead of extra text.
- Plugin pages are readable in dark mode: command boxes, the activity table, job logs and the remove panel no longer keep light backgrounds.

### Exclusions

Bundled brand icons; fetching icons from the internet; SVG icons.

### Implementation and contracts

See [D066](../discovery/D066-cached-app-discovery-and-icons.md). The icon is typed by its magic bytes (PNG, ICO, GIF, JPEG, WebP) and re-encoded in Rust; anything else is dropped. The main window's image policy allows `data:` images. The cache is validated field by field on load. An address-specific app from the cache stays disabled until this run's discovery confirms it, because the core authorises those destinations only from a fresh discovery. It builds on T035; it depends on T026 in metadata because T035 is not yet closed.

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Reopening the Apps tab shows known apps immediately, before discovery finishes.
- [ ] Apps that serve a small favicon show it; others show a letter tile.
- [ ] Opening a tile still verifies SSH and opens the isolated window.
- [ ] The dropdowns are legible in dark mode.

### Verification

`ui/remote-apps-model.test.ts`, the discovery parser tests in `crates/atlas-core/src/tunnel.rs`, `scripts/verify_native_apps.mjs` (updated for tiles); owner review on a real server.

### Evidence and closure

Not yet accepted. Close with owner acceptance on the installed build against a real server, recorded in `agent/evidence/`.

### Stop conditions

Stop if icons or the cache would need data beyond the discovery result.

## T065 — Owner PIN for sensitive details and protected views

<!-- atlas-ticket: {"id":"T065","feature":"F27","status":"in_progress","depends_on":["T026"],"evidence":null} -->

### Outcome and included work

An optional InfraWerf PIN, set in Settings → PIN. While it is set and locked, host key fingerprints and application details are hidden, and Files, Terminal, live workload details and application windows refuse to open until the PIN is entered. One entry unlocks for 5 minutes; "Lock now" ends it early. Decision and limits in [D067](../discovery/D067-owner-pin.md).

### Exclusions

Windows Hello; encrypting data at rest; hiding server names or the map.

### Implementation and contracts

`atlas-core/src/pin.rs` (hash, unlock timer, attempt limit, `pin_required` error) and `src-tauri/src/workspace/pin.rs` (`pin.json` beside the database). Commands `pin_status`, `pin_unlock`, `pin_lock`, `pin_set`, `pin_clear` are main-window only. `files_open`, `files_terminal_start`, `workload_inspect`, `tunnel_open` and `pack_app_open` call `require_pin`. The UI transport asks for the PIN on `pin_required` and retries once (`ui/pin-model.ts`); `Secret` masks values on screen (`ui/pin.tsx`).

**Ticket dependencies:** [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] With a PIN set and locked, the protected commands refuse until the PIN is entered, then work for 5 minutes.
- [ ] Fingerprints and app details are masked until unlocked and hide again when the unlock ends.
- [ ] Only a salted hash is stored; wrong PINs are rate limited; removing `pin.json` removes the PIN.
- [ ] Without a PIN, nothing changes.

### Verification

`crates/atlas-core/src/pin.rs` tests; `ui/pin-model.test.ts`; owner review on the installed build.

### Evidence and closure

Not yet accepted. Close with owner acceptance on the installed build against a real server, recorded in `agent/evidence/`.

### Stop conditions

Stop if the gate would need to store the PIN itself or change the database.
