# D056 — A categorised main menu with detachable category copies

Date: 2026-11-14. Related: S12 / F18 T053; [D055](D055-plugin-choices-belong-to-the-owner-in-settings.md).

## Owner decision

The owner asked on 2026-11-14 for the main menu to work like the settings navigation of large applications such as Cloudflare, with the floating-window behaviour of Transport Tycoon:

1. Menu items sit in categories, and the menu is **never more than one level deep**.
2. Each category **can be detached** from the main menu as a **copy** of that menu and dragged anywhere inside the application window.
3. The main menu is **retractable**, with a **permanently floating button** that brings it back.
4. It should include the normal behaviour expected of such a menu system.

## What this means

- **Categories:** Map (Topology, Inventory), Operations (Transfers, Activity) and Manage (Servers & monitoring, Settings). The middle category is named Operations, not Activity, so a category never shares a name with one of its items.
- **Menu items are registered contributions** at a new `menu_item` extension point, like tabs and settings sections, so a plugin can later add an item. An item only opens a view and may declare only `read_saved_state`. The interface decides which category an item sits in; an item that names an unknown category is rejected, which is what keeps the menu one level deep.
- **Behaviour taken as "normal" for this kind of menu:** a detached copy has a title bar to drag it by, comes to the front when used, snaps to the window edges, cannot leave the window (including after the window is resized), rolls up to its title bar (button or double-click), closes (button or Escape), and moves with the arrow keys. A category has at most one copy; its button in the main menu shows the state and docks it again. A category can also be torn off by dragging that button out of the menu. Categories collapse in the main menu. Ctrl+B hides or shows the main menu. The whole layout is remembered on this PC, and Settings has a section that resets it.
- **Scope note.** `SCOPE.md` excludes a "floating-window desktop" and a "visual redesign". This decision narrowly widens that: floating copies of menu categories inside the single application window, and the navigation rail becoming a labelled menu. Separate operating-system windows, floating content windows and a restyle of the rest of the application stay excluded.

## Still open

- Whether views other than menus (for example a server workspace) should ever float. Not planned.
- The layout is stored in the WebView's local storage, like remote-app shortcuts, not in the native database. It is a display preference; nothing else depends on it.

Update 2026-11-18: detaching categories is disabled; see [D064](D064-menu-pop-outs-disabled.md).
