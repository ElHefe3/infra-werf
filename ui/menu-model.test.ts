import test from "node:test";
import assert from "node:assert/strict";
import { cascade, closePanel, defaultLayout, detach, groupMenu, loadLayout, MENU_STORAGE_KEY, MENU_WIDTH, movePanel, parseLayout, place, raise, saveLayout, setOpen, toggleCollapsed, toggleShade, type MenuRenderer } from "./menu-model.ts";
import type { Bound } from "./extensions-model.ts";

const item = (id: string, order: number, category: string): Bound<MenuRenderer> =>
  ({ contribution: { id, point: "menu_item", title: id.split(".").at(-1)!, owner: "builtin", order, capabilities: ["read_saved_state"] }, renderer: { category, icon: "·", view: "topology" } });

test("items are grouped one level deep, in category then registry order", () => {
  const groups = groupMenu([item("builtin.menu.topology", 10, "map"), item("builtin.menu.inventory", 20, "map"), item("builtin.menu.settings", 60, "manage")]);
  assert.deepEqual(groups.map(g => [g.id, g.items.map(i => i.title)]), [["map", ["topology", "inventory"]], ["manage", ["settings"]]]);
  // A category without items is not shown, and an item cannot invent a category.
  assert.ok(!groups.some(g => g.id === "operations"));
  assert.throws(() => groupMenu([item("rogue.menu.extra", 1, "nested/deeper")]), /unknown category/);
  // An entry is a view, never another menu.
  assert.deepEqual(Object.keys(groups[0].items[0]).sort(), ["icon", "id", "title", "view"]);
});

test("a stored layout is validated before use", () => {
  assert.deepEqual(parseLayout(null), defaultLayout());
  assert.deepEqual(parseLayout("not json"), defaultLayout());
  assert.deepEqual(parseLayout("[1]"), defaultLayout());
  const stored = JSON.stringify({ open: false, collapsed: ["map", "map", "gone", 4], panels: [
    { category: "map", x: 10.4, y: 20, shaded: true }, { category: "map", x: 1, y: 1 }, { category: "gone", x: 1, y: 1 },
    { category: "manage", x: "1", y: 2 }, { category: "operations", x: Infinity, y: 2 }, null, { category: "manage", x: 5, y: 6 }] });
  assert.deepEqual(parseLayout(stored), { open: false, collapsed: ["map"], panels: [{ category: "map", x: 10, y: 20, shaded: true }, { category: "manage", x: 5, y: 6, shaded: false }] });
});

test("the layout round-trips through storage and survives a failing store", () => {
  const values = new Map<string, string>();
  const storage = { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value); } };
  const layout = detach(setOpen(defaultLayout(), false), "map", 300, 120);
  saveLayout(layout, storage);
  assert.ok(values.has(MENU_STORAGE_KEY));
  assert.deepEqual(loadLayout(storage), layout);
  const broken = { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } };
  assert.deepEqual(loadLayout(broken), defaultLayout());
  assert.doesNotThrow(() => saveLayout(layout, broken));
});

test("a panel stays inside the window and snaps to nearby edges", () => {
  const size = { width: 200, height: 100 }, viewport = { width: 1000, height: 600 };
  assert.deepEqual(place(300, 200, size, viewport), { x: 300, y: 200 });
  assert.deepEqual(place(-500, 9000, size, viewport), { x: 0, y: 500 });
  assert.deepEqual(place(9, 492, size, viewport), { x: 0, y: 500 });
  assert.deepEqual(place(790, 13, size, viewport), { x: 800, y: 13 });
  // A window smaller than the panel pins it to the top-left corner.
  assert.deepEqual(place(50, 50, size, { width: 120, height: 80 }), { x: 0, y: 0 });
});

test("detaching makes one copy per category and manages the stacking order", () => {
  let layout = detach(defaultLayout(), "map", 10, 10);
  layout = detach(layout, "manage", 40, 40);
  assert.deepEqual(layout.panels.map(p => p.category), ["map", "manage"]);
  // Detaching again raises the existing copy without moving it.
  layout = detach(layout, "map", 999, 999);
  assert.deepEqual(layout.panels.map(p => [p.category, p.x]), [["manage", 40], ["map", 10]]);
  assert.equal(raise(layout, "map"), layout);
  layout = movePanel(layout, "manage", 70, 80);
  assert.deepEqual(layout.panels[0], { category: "manage", x: 70, y: 80, shaded: false });
  assert.equal(movePanel(layout, "manage", 70, 80), layout);
  layout = toggleShade(layout, "manage");
  assert.equal(layout.panels[0].shaded, true);
  layout = closePanel(layout, "manage");
  assert.deepEqual(layout.panels.map(p => p.category), ["map"]);
  assert.equal(closePanel(layout, "manage"), layout);
});

test("hiding the menu and collapsing a category leave detached panels alone", () => {
  let layout = detach(defaultLayout(), "map", 10, 10);
  layout = toggleCollapsed(setOpen(layout, false), "map");
  assert.deepEqual(layout, { open: false, collapsed: ["map"], panels: [{ category: "map", x: 10, y: 10, shaded: false }] });
  assert.deepEqual(toggleCollapsed(layout, "map").collapsed, []);
  assert.deepEqual(cascade(0, true), { x: MENU_WIDTH + 16, y: 76 });
  assert.deepEqual(cascade(2, false), { x: 72, y: 132 });
});
