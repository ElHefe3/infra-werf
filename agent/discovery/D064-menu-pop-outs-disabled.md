# D064 — Detachable menu categories are disabled

Date: 2026-11-18. Related: [D056](D056-categorised-detachable-main-menu.md), S12 / F18 T053.

## Owner decision

On 2026-11-18 the owner asked for the menu pop-out feature to be disabled for now: "It is not being used correctly." The pop-outs were part of the T053 main menu ([D056](D056-categorised-detachable-main-menu.md)). They let a menu category be torn off as a floating copy inside the window.

## Why it serves no purpose in its current state

- A floating copy only repeats links that are already in the main menu, two clicks away. It adds no capability and takes up screen space over the content.
- Detaching belongs with configuration, not navigation. A floating panel would be useful for keeping a setting or control at hand next to the work, not for a second route to the same views.

## What changed

- `MENU_POPOUT_ENABLED = false` in `ui/menu.tsx` hides the detach (⧉) buttons and does not draw floating panels. A layout saved with floating panels is ignored, not deleted.
- The menu still has categories that collapse, the hide button and Ctrl+B, and a remembered layout. Settings → Menu layout now describes only those, and its reset still works.
- The model code and its unit tests in `ui/menu-model.ts` stay, so the feature can come back as a configuration feature without starting from scratch.
- The desktop journey (`scripts/verify_desktop.mjs`) now checks that no detach control or floating panel appears, instead of exercising them.

## Still open

- Whether floating configuration panels should exist, and which configuration they would hold. Not planned.
