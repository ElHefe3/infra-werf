import { ExtensionBindingError, type Bound } from "./extensions-model.ts";

// Main menu (T053). Menu items are registered contributions; the interface
// places each in one category. Categories are exactly one level deep: a
// category holds items and an item never holds another menu. A category can
// be detached as a floating copy of its menu inside the application window.

export type View = "topology" | "inventory" | "transfers" | "activity" | "servers" | "settings";
export type MenuRenderer = { category: string; icon: string; view: View };
export type Category = { id: string; title: string };
export type MenuEntry = { id: string; title: string; icon: string; view: View };
export type MenuGroup = Category & { items: MenuEntry[] };

export const CATEGORIES: readonly Category[] = [
  { id: "map", title: "Map" },
  { id: "operations", title: "Operations" },
  { id: "manage", title: "Manage" },
];

/** Items under their categories, in category order then registry order. Empty categories are left out. */
export function groupMenu(items: Bound<MenuRenderer>[], categories: readonly Category[] = CATEGORIES): MenuGroup[] {
  for (const { contribution, renderer } of items) {
    if (!categories.some(category => category.id === renderer.category)) throw new ExtensionBindingError(`Menu item ${contribution.id} names the unknown category ${renderer.category}.`);
  }
  return categories.map(category => ({ ...category, items: items.filter(item => item.renderer.category === category.id).map(({ contribution, renderer }) => ({ id: contribution.id, title: contribution.title, icon: renderer.icon, view: renderer.view })) }))
    .filter(group => group.items.length);
}

/** A detached category menu. Positions are window pixels from the top-left corner. */
export type Panel = { category: string; x: number; y: number; shaded: boolean };
/** `panels` is ordered back to front: the last one is on top. */
export type MenuLayout = { open: boolean; collapsed: string[]; panels: Panel[] };
export type Size = { width: number; height: number };

export const MENU_STORAGE_KEY = "infrawerf.menu-layout.v1";
export const MENU_WIDTH = 188;
/** Distance within which a dragged panel snaps to a window edge. */
export const SNAP = 12;
export const defaultLayout = (): MenuLayout => ({ open: true, collapsed: [], panels: [] });

/** Reads a stored layout, dropping anything malformed or naming a category that no longer exists. */
export function parseLayout(raw: string | null | undefined, categories: readonly Category[] = CATEGORIES): MenuLayout {
  if (!raw) return defaultLayout();
  let value: unknown;
  try { value = JSON.parse(raw); } catch { return defaultLayout(); }
  if (!value || typeof value !== "object" || Array.isArray(value)) return defaultLayout();
  const stored = value as Record<string, unknown>;
  const known = (id: unknown): id is string => typeof id === "string" && categories.some(category => category.id === id);
  const collapsed = Array.isArray(stored.collapsed) ? [...new Set(stored.collapsed.filter(known))] : [];
  const panels: Panel[] = [];
  for (const entry of Array.isArray(stored.panels) ? stored.panels : []) {
    if (!entry || typeof entry !== "object") continue;
    const { category, x, y, shaded } = entry as Record<string, unknown>;
    if (!known(category) || panels.some(panel => panel.category === category)) continue;
    if (typeof x !== "number" || typeof y !== "number" || !Number.isFinite(x) || !Number.isFinite(y)) continue;
    panels.push({ category, x: Math.round(x), y: Math.round(y), shaded: shaded === true });
  }
  return { open: stored.open !== false, collapsed, panels };
}

export function loadLayout(storage: Pick<Storage, "getItem"> | undefined = globalThis.localStorage): MenuLayout {
  try { return parseLayout(storage?.getItem(MENU_STORAGE_KEY)); } catch { return defaultLayout(); }
}
export function saveLayout(layout: MenuLayout, storage: Pick<Storage, "setItem"> | undefined = globalThis.localStorage) {
  try { storage?.setItem(MENU_STORAGE_KEY, JSON.stringify(layout)); } catch { /* The layout still works for this run. */ }
}

/** Snaps to a nearby window edge, then keeps the whole panel inside the window. */
export function place(x: number, y: number, size: Size, viewport: Size): { x: number; y: number } {
  const axis = (value: number, extent: number, limit: number) => {
    const max = Math.max(0, limit - extent);
    if (Math.abs(value) <= SNAP) return 0;
    if (Math.abs(max - value) <= SNAP) return max;
    return Math.round(Math.min(Math.max(value, 0), max));
  };
  return { x: axis(x, size.width, viewport.width), y: axis(y, size.height, viewport.height) };
}

/** Where a newly detached panel appears: beside the menu, stepping down and right for each one already open. */
export function cascade(count: number, menuOpen: boolean): { x: number; y: number } {
  return { x: (menuOpen ? MENU_WIDTH : 0) + 16 + 28 * count, y: 76 + 28 * count };
}

const without = (layout: MenuLayout, category: string) => layout.panels.filter(panel => panel.category !== category);

/** Brings a panel to the front. */
export function raise(layout: MenuLayout, category: string): MenuLayout {
  const panel = layout.panels.find(entry => entry.category === category);
  if (!panel || layout.panels.at(-1) === panel) return layout;
  return { ...layout, panels: [...without(layout, category), panel] };
}
/** Detaches a category as a floating copy. A category has at most one copy; detaching it again raises it. */
export function detach(layout: MenuLayout, category: string, x: number, y: number): MenuLayout {
  if (layout.panels.some(panel => panel.category === category)) return raise(layout, category);
  return { ...layout, panels: [...layout.panels, { category, x: Math.round(x), y: Math.round(y), shaded: false }] };
}
export function closePanel(layout: MenuLayout, category: string): MenuLayout {
  return layout.panels.some(panel => panel.category === category) ? { ...layout, panels: without(layout, category) } : layout;
}
export function movePanel(layout: MenuLayout, category: string, x: number, y: number): MenuLayout {
  const panel = layout.panels.find(entry => entry.category === category);
  if (!panel || (panel.x === x && panel.y === y)) return layout;
  return { ...layout, panels: layout.panels.map(entry => entry === panel ? { ...entry, x, y } : entry) };
}
/** Rolls a panel up to its title bar, or back down. */
export function toggleShade(layout: MenuLayout, category: string): MenuLayout {
  return { ...layout, panels: layout.panels.map(panel => panel.category === category ? { ...panel, shaded: !panel.shaded } : panel) };
}
/** Collapses or expands a category in the main menu. */
export function toggleCollapsed(layout: MenuLayout, category: string): MenuLayout {
  return { ...layout, collapsed: layout.collapsed.includes(category) ? layout.collapsed.filter(id => id !== category) : [...layout.collapsed, category] };
}
export function setOpen(layout: MenuLayout, open: boolean): MenuLayout {
  return layout.open === open ? layout : { ...layout, open };
}
