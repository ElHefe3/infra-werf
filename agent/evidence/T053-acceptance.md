# T053 acceptance — categorised main menu with detachable category copies

Date: 2026-11-15. Feature: F18 / S12. Ticket: T053.
Revision: 81192ef
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `81192ef` records.

## What was built

- Core: `ExtensionPoint::MenuItem`, which may declare only `read_saved_state`. `builtin()` registers six items (`builtin.menu.topology`, `inventory`, `transfers`, `activity`, `servers`, `settings`) and a third application settings section, `builtin.app-settings.menu-layout`.
- Interface: `ui/menu-model.ts` (categories, grouping, layout validation, placement, stacking) and `ui/menu.tsx` (the menu, the floating copies and the hide/show button). Categories: Map, Operations, Manage.
- Behaviour: detach by button or by dragging the button out; drag by the title bar; snap to window edges and stay inside the window, also after a resize; bring to front on use; roll up (button, double-click, Enter); close (button, Escape); move with arrow keys; one copy per category with a dock toggle; collapsible categories; Ctrl+B and an always-present button hide and show the main menu; layout remembered in local storage; "Reset menu layout" in Settings.

## Checks on 81192ef

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 94 passed, 2 ignored (real-SSH fixture tests) |
| UI tests | pass: 27/27, including six new menu tests |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 81192ef

All twelve pass in a single run: desktop, ssh, health, inventory, files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps. The desktop suite includes the menu journey and reports: "main menu groups items in one-level categories; a category detaches as a draggable, edge-snapping, roll-up copy that is remembered; the menu hides and the floating button or Ctrl+B restores it; Settings resets the layout". The two viewport-fit checks (1280×720 and 1440×900) pass with the wider menu.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Every item is a registry entry under exactly one category; one level deep | Registry test lists the six items; binding rejects an unregistered renderer; `groupMenu` rejects an unknown category (unit test); the journey checks each category's items and that no list is nested. |
| A category detaches as a copy, drags anywhere, never leaves the window | The journey detaches Map, sees the main menu still list it, drags it to an exact position, drags it past the corner and finds it flush with the window edges. `place` is unit-tested. |
| The menu hides; one button brings it back; copies keep working | The journey hides the menu, uses the floating copy to change view, and restores the menu with Ctrl+B; the button is asserted in both states. |
| Front on use, roll up, close, keyboard, remembered, reset | The journey checks stacking order after a tear-off and after using the other panel, rolls up and unrolls, moves with arrow keys, closes with Escape, reloads the interface and finds the same position and hidden menu, and resets from Settings. |
| All native suites pass | See above. No other suite needed a change: item names are unchanged and the main menu is first in the document. |

## Notes

- The first desktop run failed because a category and an item were both named "Activity". The category was renamed Operations and category headers were given their own labels ("Map category"). This is recorded in D056.
- Not verified by a test: double-click roll-up, the window-resize containment path in the real window, and the narrow (under 900 px) layout. They share code with tested paths.
- Review: no separate review pass was run for this ticket.
