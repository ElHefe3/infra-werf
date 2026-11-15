import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { BrandMark } from "./brand";
import { cascade, closePanel, defaultLayout, detach, loadLayout, movePanel, place, raise, saveLayout, setOpen, toggleCollapsed, toggleShade, type MenuGroup, type MenuLayout, type Panel, type Size, type View } from "./menu-model";

// Main menu (T053): categories of items, one level deep. Each category can be
// detached as a floating copy that is dragged anywhere inside the window, and
// the main menu can be hidden; one floating button always brings it back.

export type MenuController = { layout: MenuLayout; update: (change: (layout: MenuLayout) => MenuLayout) => void; reset: () => void };

/** The saved menu layout for this PC. */
export function useMenu(): MenuController {
  const [layout, setLayout] = useState<MenuLayout>(() => loadLayout());
  useEffect(() => saveLayout(layout), [layout]);
  const update = useCallback((change: (layout: MenuLayout) => MenuLayout) => setLayout(change), []);
  const reset = useCallback(() => setLayout(defaultLayout()), []);
  return { layout, update, reset };
}

const viewport = (): Size => ({ width: window.innerWidth, height: window.innerHeight });
const FALLBACK: Size = { width: 196, height: 40 };
/** Where the pointer holds a panel that is being torn off the main menu. */
const GRIP = { x: 90, y: 16 };
const STEP: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };

function Items({ group, view, go }: { group: MenuGroup; view: View; go: (view: View) => void }) {
  return <ul className="menu-items">{group.items.map(item => <li key={item.id}><button className={view === item.view ? "active" : ""} aria-current={view === item.view ? "page" : undefined} onClick={() => go(item.view)}><span className="menu-icon" aria-hidden>{item.icon}</span>{item.title}</button></li>)}</ul>;
}

export function MenuSystem({ groups, view, go, menu, quit }: { groups: MenuGroup[]; view: View; go: (view: View) => void; menu: MenuController; quit: () => void }) {
  const { layout, update } = menu;
  const elements = useRef(new Map<string, HTMLElement>());
  const suppressClick = useRef(false);
  const sizeOf = (category: string): Size => { const box = elements.current.get(category)?.getBoundingClientRect(); return box ? { width: box.width, height: box.height } : FALLBACK; };
  const moveTo = (category: string, x: number, y: number) => { const next = place(x, y, sizeOf(category), viewport()); update(current => movePanel(current, category, next.x, next.y)); };

  // Panels never leave the window: after a restore, a roll-up or a window resize.
  const contain = useCallback(() => update(current => current.panels.reduce((next, panel) => {
    const box = elements.current.get(panel.category)?.getBoundingClientRect();
    if (!box) return next;
    const spot = place(panel.x, panel.y, { width: box.width, height: box.height }, viewport());
    return movePanel(next, panel.category, spot.x, spot.y);
  }, current)), [update]);
  useLayoutEffect(contain, [contain, layout.panels.length, layout.panels.map(panel => panel.shaded).join()]);
  useEffect(() => { window.addEventListener("resize", contain); return () => window.removeEventListener("resize", contain); }, [contain]);

  // Ctrl+B hides or shows the main menu, as in most desktop applications.
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey || event.key.toLowerCase() !== "b" || document.querySelector("dialog[open]")) return;
      event.preventDefault(); update(current => setOpen(current, !current.open));
    };
    window.addEventListener("keydown", handler); return () => window.removeEventListener("keydown", handler);
  }, [update]);

  const detached = (category: string) => layout.panels.some(panel => panel.category === category);
  function toggleDetached(category: string) {
    update(current => { if (current.panels.some(panel => panel.category === category)) return closePanel(current, category); const spot = cascade(current.panels.length, current.open); return detach(current, category, spot.x, spot.y); });
  }

  // A drag is followed on the window, not on the element under the pointer,
  // so it survives re-renders and the pointer leaving the title bar.
  function track(move: (event: PointerEvent) => void, end: () => void) {
    const stop = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", stop); window.removeEventListener("pointercancel", stop); end(); };
    window.addEventListener("pointermove", move); window.addEventListener("pointerup", stop); window.addEventListener("pointercancel", stop);
  }

  // Dragging the detach button out of the menu tears the category off under the pointer.
  function tearDown(event: React.PointerEvent<HTMLButtonElement>, category: string) {
    if (event.button !== 0 || detached(category)) return;
    const state = { x: event.clientX, y: event.clientY, torn: false };
    track(move => {
      if (!state.torn && Math.hypot(move.clientX - state.x, move.clientY - state.y) < 6) return;
      const x = move.clientX - GRIP.x, y = move.clientY - GRIP.y;
      if (!state.torn) { state.torn = true; const spot = place(x, y, FALLBACK, viewport()); update(current => detach(current, category, spot.x, spot.y)); }
      else moveTo(category, x, y);
    }, () => { if (state.torn) { suppressClick.current = true; window.setTimeout(() => { suppressClick.current = false; }, 0); } });
  }

  function titleDown(event: React.PointerEvent<HTMLElement>, panel: Panel) {
    if (event.button !== 0 || (event.target as HTMLElement).closest("button")) return;
    // No text selection or native drag may take the pointer away mid-move.
    event.preventDefault(); event.currentTarget.focus();
    const dx = event.clientX - panel.x, dy = event.clientY - panel.y;
    track(move => moveTo(panel.category, move.clientX - dx, move.clientY - dy), () => {});
  }
  function titleKey(event: React.KeyboardEvent<HTMLElement>, panel: Panel) {
    if (event.target !== event.currentTarget) return;
    const step = STEP[event.key];
    if (step) { event.preventDefault(); const size = event.shiftKey ? 64 : 16; moveTo(panel.category, panel.x + step[0] * size, panel.y + step[1] * size); }
    else if (event.key === "Enter" || event.key === " ") { event.preventDefault(); update(current => toggleShade(current, panel.category)); }
  }

  return <>
    <button className="menu-toggle" aria-label={layout.open ? "Hide menu" : "Show menu"} title={`${layout.open ? "Hide" : "Show"} menu (Ctrl+B)`} aria-expanded={layout.open} aria-controls="main-menu" onClick={() => update(current => setOpen(current, !current.open))}>{layout.open ? "⟨" : "☰"}</button>
    {layout.open && <aside className="rail" id="main-menu"><div className="rail-head"><BrandMark /></div>
      <nav aria-label="Main navigation">{groups.map(group => { const collapsed = layout.collapsed.includes(group.id); const floating = detached(group.id); return <section className="menu-category" key={group.id} aria-label={group.title}>
        <div className="menu-category-head"><button className="menu-category-toggle" aria-label={`${group.title} category`} aria-expanded={!collapsed} onClick={() => update(current => toggleCollapsed(current, group.id))}><span aria-hidden>{collapsed ? "▸" : "▾"}</span>{group.title}</button>
          <button className="menu-detach" aria-label={`${floating ? "Dock" : "Detach"} ${group.title} menu`} title={floating ? `Dock the floating ${group.title} menu` : `Detach ${group.title} as a floating menu (click or drag out)`} aria-pressed={floating} onPointerDown={event => tearDown(event, group.id)} onClick={() => { if (suppressClick.current) { suppressClick.current = false; return; } toggleDetached(group.id); }}>⧉</button></div>
        {!collapsed && <Items group={group} view={view} go={go} />}
      </section>; })}</nav>
      <div className="rail-bottom"><button className="quit-button" aria-label="Quit InfraWerf" title="Quit InfraWerf" onClick={quit}><span className="menu-icon" aria-hidden>⏻</span>Quit</button><span className="local-avatar" title="Local workspace">L</span></div>
    </aside>}
    {layout.panels.map((panel, index) => { const group = groups.find(entry => entry.id === panel.category); if (!group) return null; return <section className={`menu-panel ${panel.shaded ? "shaded" : ""}`} key={panel.category} role="dialog" aria-label={`${group.title} menu`} style={{ left: panel.x, top: panel.y, zIndex: 40 + index }}
      ref={element => { if (element) elements.current.set(panel.category, element); else elements.current.delete(panel.category); }}
      onPointerDownCapture={() => update(current => raise(current, panel.category))}
      onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); update(current => closePanel(current, panel.category)); } }}>
      <header className="menu-panel-title" tabIndex={0} aria-label={`Move ${group.title} menu`} aria-roledescription="draggable title bar" title="Drag to move · double-click to roll up · arrow keys move" onPointerDown={event => titleDown(event, panel)} onDragStart={event => event.preventDefault()} onDoubleClick={event => { if (!(event.target as HTMLElement).closest("button")) update(current => toggleShade(current, panel.category)); }} onKeyDown={event => titleKey(event, panel)}>
        <span>{group.title}</span>
        <button aria-label={`${panel.shaded ? "Unroll" : "Roll up"} ${group.title} menu`} aria-expanded={!panel.shaded} title={panel.shaded ? "Unroll" : "Roll up"} onClick={() => update(current => toggleShade(current, panel.category))}>{panel.shaded ? "▾" : "▴"}</button>
        <button aria-label={`Close ${group.title} menu`} title="Close (Esc)" onClick={() => update(current => closePanel(current, panel.category))}>×</button>
      </header>
      {!panel.shaded && <Items group={group} view={view} go={go} />}
    </section>; })}
  </>;
}
