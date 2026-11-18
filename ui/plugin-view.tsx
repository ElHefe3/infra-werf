import React, { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { MapServer, PluginViewTab, ViewActionRequest } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { Dialog } from "./dialog";
const client = createWorkspaceClient();

// Plugin views (S12/F23/T047). The page itself runs in a separate webview
// on its own origin, laid over the placeholder below; this file only places
// it, tears it down, and shows InfraWerf's own dialog when a view asks to
// run a command. Nothing a plugin page does reaches this code directly.

/** Workspace tab IDs for plugin views. */
export const viewTabId = (view: Pick<PluginViewTab, "plugin_id" | "contribution_id">) => `plugin-view:${view.plugin_id}/${view.contribution_id}`;

/** Asks every open view to hide (while InfraWerf shows something over it) or come back. */
const VIEWS_EVENT = "infrawerf-plugin-views";
export const hideViews = (hidden: boolean) => window.dispatchEvent(new CustomEvent(VIEWS_EVENT, { detail: hidden }));

export function usePluginViews(serverId: string): PluginViewTab[] {
  const [views, setViews] = useState<PluginViewTab[]>([]);
  useEffect(() => { let live = true; client.pluginViews(serverId).then(state => { if (live) setViews(state.views); }, () => {}); return () => { live = false; }; }, [serverId]);
  return views;
}

export function PluginViewPanel({ server, view }: { server: MapServer; view: PluginViewTab }) {
  const area = useRef<HTMLDivElement>(null);
  const label = useRef<string | null>(null);
  const hidden = useRef(false);
  const [state, setState] = useState<"opening" | "open" | "locked" | "closed" | "failed">("opening");
  const [failure, setFailure] = useState("");
  const bounds = useCallback(() => {
    const rect = area.current?.getBoundingClientRect();
    return rect ? { x: rect.left, y: rect.top, width: rect.width, height: rect.height } : null;
  }, []);
  const place = useCallback(() => {
    const id = label.current, box = bounds();
    if (id && box) void client.pluginViewBounds(id, hidden.current ? null : box).catch(() => {});
  }, [bounds]);
  const close = useCallback(async (next: "locked" | "closed") => {
    const id = label.current; label.current = null;
    if (id) await client.closePluginView(id).catch(() => {});
    setState(next);
  }, []);
  const open = useCallback(async () => {
    const box = bounds();
    if (!box) return;
    setState("opening"); setFailure("");
    try { label.current = await client.openPluginView(server.id, view.plugin_id, view.contribution_id, box); setState("open"); }
    catch (error) { setFailure(errorMessage(error)); setState("failed"); }
  }, [bounds, server.id, view.plugin_id, view.contribution_id]);
  useEffect(() => {
    void open();
    let frame = 0;
    const later = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(place); };
    const observer = new ResizeObserver(later);
    if (area.current) observer.observe(area.current);
    window.addEventListener("resize", later); window.addEventListener("scroll", later, true);
    const onViews = (event: Event) => { hidden.current = Boolean((event as CustomEvent<boolean>).detail); place(); };
    window.addEventListener(VIEWS_EVENT, onViews);
    // InfraWerf closes views itself when plugin permissions change.
    const watch = window.setInterval(async () => {
      const id = label.current;
      if (!id) return;
      const open = await client.openPluginViews().catch(() => null);
      if (open && !open.some(entry => entry.label === id)) { label.current = null; setState("closed"); }
    }, 1500);
    return () => {
      observer.disconnect(); cancelAnimationFrame(frame); window.clearInterval(watch);
      window.removeEventListener("resize", later); window.removeEventListener("scroll", later, true); window.removeEventListener(VIEWS_EVENT, onViews);
      const id = label.current; label.current = null;
      if (id) void client.closePluginView(id).catch(() => {});
    };
  }, [open, place]);
  return <section className="plugin-view" aria-label={`${view.title} from ${view.plugin_name}`}>
    <header className="plugin-view-bar"><span>{view.title} · from <strong>{view.plugin_name}</strong> · runs isolated from InfraWerf, with only what you allowed on {server.name}</span>
      {state === "open" ? <button onClick={() => void close("locked")}>Lock view</button> : state !== "opening" && <button onClick={() => void open()}>Open view again</button>}</header>
    <div ref={area} className="plugin-view-area" aria-label={`${view.title} view`} data-state={state}>
      {state === "locked" && <p className="empty-copy">View locked. The plugin page was closed and its requests cancelled.</p>}
      {state === "closed" && <p className="empty-copy">This view was closed because what plugins may do changed. Open it again if the plugin is still allowed here.</p>}
      {state === "failed" && <p className="form-error" role="alert">The view could not open: {failure} Check the plugin’s page in Settings.</p>}
    </div>
  </section>;
}

/** How long Run stays disabled after the dialog appears, so a click aimed at the plugin page cannot land on it (T047 review, finding 3). */
export const RUN_DELAY_MS = 1000;

/** InfraWerf's own dialog for a command a plugin view asks to run. Requests from several views queue; one is shown at a time. */
export function PluginActionDialog() {
  const [queue, setQueue] = useState<ViewActionRequest[]>([]);
  const [busy, setBusy] = useState(false);
  const [ready, setReady] = useState(false);
  const pressed = useRef(false);
  const decline = useRef<HTMLButtonElement>(null);
  const request = queue[0] ?? null;
  useEffect(() => {
    let disposed = false, unlisten: (() => void) | undefined;
    void listen<ViewActionRequest>("atlas-plugin-action", event => { hideViews(true); setQueue(current => current.some(r => r.id === event.payload.id) ? current : [...current, event.payload]); })
      .then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {});
    return () => { disposed = true; unlisten?.(); };
  }, []);
  // Each request, and every return to the window, starts the wait again.
  useEffect(() => {
    if (!request) return;
    setReady(false); pressed.current = false;
    // A stray Enter or Space declines.
    requestAnimationFrame(() => decline.current?.focus());
    let timer = window.setTimeout(() => setReady(true), RUN_DELAY_MS);
    const restart = () => { setReady(false); window.clearTimeout(timer); timer = window.setTimeout(() => setReady(true), RUN_DELAY_MS); };
    window.addEventListener("focus", restart);
    return () => { window.clearTimeout(timer); window.removeEventListener("focus", restart); };
  }, [request?.id]);
  if (!request) return null;
  async function answer(run: boolean) {
    if (!request) return;
    setBusy(true);
    try { await client.answerPluginView(request.id, run); } catch { /* the view sees the request's state */ }
    finally {
      setBusy(false);
      setQueue(current => { const rest = current.slice(1); if (!rest.length) hideViews(false); return rest; });
    }
  }
  const chosen = Object.entries(request.parameters);
  return <Dialog title="A plugin asks to run a command" close={() => void answer(false)} busy={busy}>
    <p><strong>{request.plugin_name}</strong> asks to run <strong>{request.title}</strong> on <strong>{request.server_name}</strong>.</p>
    <code className="pack-argv" aria-label="Requested command">{request.argv.join(" ")}</code>
    {chosen.length > 0 && <ul aria-label="Chosen workloads">{chosen.map(([name, value]) => <li key={name}><code>{`{${name}}`}</code> = {value}</li>)}</ul>}
    <p className="muted">This request came from the plugin’s own page. It runs only if you press Run here, through your SSH access and the same checks as the Plugin data tab. Nothing else in the plugin page can run it.</p>
    {queue.length > 1 && <p className="muted">{queue.length - 1} more {queue.length === 2 ? "request is" : "requests are"} waiting after this one.</p>}
    <div className="form-actions"><button ref={decline} disabled={busy} onClick={() => void answer(false)}>Decline</button>
      <button className="primary" disabled={busy || !ready} onPointerDown={() => { pressed.current = true; }}
        onClick={event => { const fromPointer = event.detail > 0; if (fromPointer && !pressed.current) return; pressed.current = false; void answer(true); }}>{busy ? "Running…" : ready ? "Run" : "Run (wait…)"}</button></div>
  </Dialog>;
}
