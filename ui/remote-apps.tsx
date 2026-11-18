import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { AppDiscovery, MapServer, PackAppView, TunnelInfo } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { type AppShortcut, type CachedDiscovery, appHue, appKey, appName, describeApp, loadDiscovery, loadShortcuts, parseTarget, saveDiscovery, saveShortcuts } from "./remote-apps-model";
import type { DiscoveredApp } from "./contracts.gen";
import { usePin } from "./pin";
const client = createWorkspaceClient();

/** F17/T035: open a server's localhost web interfaces in isolated windows. */
export function RemoteApps({ server, disabled }: { server: MapServer; disabled: boolean }) {
  const [tunnels, setTunnels] = useState<TunnelInfo[]>([]);
  const [shortcuts, setShortcuts] = useState<AppShortcut[]>(() => loadShortcuts(server.id));
  const [port, setPort] = useState(""), [path, setPath] = useState("/"), [name, setName] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [notice, setNotice] = useState("");
  // The last discovery shows at once; a fresh one replaces it when it arrives.
  const [discovery, setDiscovery] = useState<CachedDiscovery | null>(() => loadDiscovery(server.id)), [discovering, setDiscovering] = useState(false), [fresh, setFresh] = useState(false);
  const [packApps, setPackApps] = useState<PackAppView[]>([]);
  async function discover() {
    setDiscovering(true); setError("");
    try {
      const found: AppDiscovery = await client.discoverApps(server.id);
      setDiscovery(found); setFresh(true); saveDiscovery(server.id, found);
    } catch (e) { setError(errorMessage(e)); } finally { setDiscovering(false); }
  }
  async function refresh() { try { setTunnels((await client.tunnels()).filter(t => t.server_id === server.id)); } catch (e) { setError(errorMessage(e)); } }
  useEffect(() => {
    let disposed = false, unlisten: (() => void) | undefined;
    void refresh(); if (!disabled) void discover();
    void client.packApps(server.id).then(found => { if (!disposed) setPackApps(found.apps); }, () => {});
    void listen("atlas-tunnels-changed", () => void refresh()).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {});
    const timer = setInterval(() => void refresh(), 5000);
    return () => { disposed = true; clearInterval(timer); unlisten?.(); };
  }, [server.id]);
  async function open(target: { port: number; path: string; name?: string; remote_host?: string | null }) {
    setBusy(true); setError(""); setNotice("");
    try {
      const info = await client.openTunnel(server.id, { remote_port: target.port, path: target.path, ...(target.remote_host ? { remote_host: target.remote_host } : {}) });
      setNotice(`${target.name || `${target.remote_host ?? "localhost"}:${target.port}`} opened in its own window through a private tunnel.`);
      setTunnels(current => [...current.filter(t => t.tunnel_id !== info.tunnel_id), info]);
    } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
  }
  async function openPack(app: PackAppView) {
    setBusy(true); setError(""); setNotice("");
    try {
      const info = await client.openPackApp(server.id, app.plugin_id, app.contribution_id);
      setNotice(`${app.title} opened in its own window through a private tunnel.`);
      setTunnels(current => [...current.filter(t => t.tunnel_id !== info.tunnel_id), info]);
    } catch (e) {
      const code = typeof e === "object" && e && "code" in e ? (e as { code: string }).code : "";
      setError(code === "denied" ? `InfraWerf refused to open ${app.title}: ${app.plugin_name} is no longer allowed to open applications on ${server.name}. Check the plugin’s page in Settings.` : errorMessage(e));
      void client.packApps(server.id).then(found => setPackApps(found.apps), () => {});
    } finally { setBusy(false); }
  }
  function submit(event: React.FormEvent) {
    event.preventDefault();
    const target = parseTarget(port, path);
    if (!target) { setError("Enter a port from 1 to 65535 and a path starting with /."); return; }
    void open({ ...target, name });
  }
  function remember() {
    const target = parseTarget(port, path);
    if (!target || !name.trim()) { setError("Name the application and enter a valid port and path to save a shortcut."); return; }
    const next = [...shortcuts.filter(s => s.name !== name.trim()), { name: name.trim().slice(0, 60), ...target }];
    setShortcuts(next); saveShortcuts(server.id, next); setNotice(`Saved “${name.trim()}”.`); setError("");
  }
  function forget(item: AppShortcut) { const next = shortcuts.filter(s => s !== item); setShortcuts(next); saveShortcuts(server.id, next); }
  async function close(id: string) { try { await client.closeTunnel(id); await refresh(); } catch (e) { setError(errorMessage(e)); } }
  const locked = disabled || busy;
  const openable = discovery?.apps.filter(app => describeApp(app).openable) ?? [];
  const others = discovery?.apps.filter(app => !describeApp(app).openable) ?? [];
  return <div className="remote-apps">
    <div className="section-heading"><div><h3>Remote applications</h3><p>Open web interfaces that listen only on {server.name}'s localhost, in their own InfraWerf window.</p></div>
      {tunnels.length > 0 && <button className="danger text-button" onClick={() => void client.closeAllTunnels().then(refresh).catch(e => setError(errorMessage(e)))}>Close all tunnels</button>}</div>
    {error && <p className="form-error" role="alert">{error}</p>}{notice && <p role="status" className="apps-notice">{notice}</p>}
    <div className="apps-heading"><p className="section-label">ON {server.name.toUpperCase()}</p>
      <span className="muted">{discovering ? (discovery ? "Checking for changes…" : "Looking for applications…") : discovery ? `Checked ${new Date(discovery.discovered_at).toLocaleString()}` : "Not checked yet"}</span>
      <button disabled={locked || discovering} onClick={() => void discover()}>Refresh</button></div>
    {discovery && !openable.length && !discovering && <p className="muted">No web applications found on {server.name}.</p>}
    {openable.length > 0 && <div className="app-grid" aria-label="Detected applications">{openable.map(app => {
      const waiting = Boolean(app.remote_host) && !fresh;
      return <AppTile key={appKey(app)} app={app} disabled={locked || waiting} note={waiting ? "Waiting for discovery to confirm this address" : undefined} details={[["Address", appKey(app)], ["Process", app.process], ["Page title", app.title], ["Version", app.version], ["HTTP status", app.http_status === null ? null : String(app.http_status)]]} open={() => void open({ port: app.port, path: "/", name: appName(app), remote_host: app.remote_host })} />;
    })}</div>}
    {discovery && !discovery.complete && <p className="muted">The server has no ss tool, so the list may be incomplete.</p>}
    {others.length > 0 && <details className="app-others"><summary>Other listening ports ({others.length})</summary><ul>{others.map(app => <li key={appKey(app)}>{describeApp(app).label}</li>)}</ul></details>}
    {packApps.length > 0 && <><p className="section-label">FROM PLUGINS</p><div className="app-grid" aria-label="Applications from plugins">{packApps.map(app => <AppTile key={`${app.plugin_id}.${app.contribution_id}`} app={{ port: app.port, title: app.title, process: null, remote_host: null, icon: null }} details={[["Address", `localhost:${app.port}${app.path !== "/" ? app.path : ""}`], ["From plugin", app.plugin_name]]} label={`Open ${app.title}`} disabled={locked} open={() => void openPack(app)} />)}</div></>}
    {shortcuts.length > 0 && <><p className="section-label">SAVED APPLICATIONS</p><div className="app-grid">{shortcuts.map(item => <div className="app-shortcut" key={item.name}>
      <AppTile app={{ port: item.port, title: item.name, process: null, remote_host: null, icon: null }} details={[["Address", `localhost:${item.port}${item.path !== "/" ? item.path : ""}`]]} label={item.name} disabled={locked} open={() => void open(item)} />
      <button className="text-button" aria-label={`Forget ${item.name}`} onClick={() => forget(item)}>Forget</button></div>)}</div></>}
    <details className="app-manual"><summary>Enter a port manually</summary>
    <form className="app-form" onSubmit={submit}>
      <label>Remote port<input aria-label="Remote port" inputMode="numeric" required placeholder="e.g. 3000" value={port} onChange={e => setPort(e.target.value)} /></label>
      <label>Path<input aria-label="Path" value={path} spellCheck={false} onChange={e => setPath(e.target.value)} /></label>
      <label>Name (for a shortcut)<input aria-label="Application name" maxLength={60} placeholder="e.g. Grafana" value={name} onChange={e => setName(e.target.value)} /></label>
      <div className="app-form-actions"><button type="button" disabled={locked} onClick={remember}>Save shortcut</button><button className="primary" disabled={locked}>{busy ? "Verifying and connecting…" : "Open application"}</button></div>
    </form></details>
    <p className="section-label">OPEN TUNNELS</p>
    {tunnels.length ? <ul className="tunnel-list">{tunnels.map(t => <li key={t.tunnel_id}><div><strong>localhost:{t.remote_port}</strong><span>via 127.0.0.1:{t.local_port} · opened {new Date(t.opened_at).toLocaleTimeString()} · closes by {new Date(t.expires_at).toLocaleTimeString()}</span></div>
      <div className="tunnel-actions"><button onClick={() => void client.focusTunnel(t.tunnel_id).catch(e => setError(errorMessage(e)))}>Show window</button><button className="danger text-button" onClick={() => void close(t.tunnel_id)}>Close</button></div></li>)}</ul>
      : <p className="muted">No tunnels are open for this server.</p>}
    <p className="muted">InfraWerf first verifies SSH access with your existing OpenSSH trust, then forwards a random port on this PC (127.0.0.1 only) to localhost on the server. Nothing is installed or exposed on your network. Application windows run privately, cannot control InfraWerf, and stay on the tunnel address. Closing the window, archiving or pausing the server, or quitting InfraWerf ends the tunnel; tunnels also close after four hours.</p>
  </div>;
}

type TileApp = Pick<DiscoveredApp, "port" | "title" | "process" | "remote_host" | "icon">;

/** One application tile: its name and icon (or a letter). Address, process,
 * version and status sit behind the ⓘ button, which needs the PIN when set. */
function AppTile({ app, open, disabled, note, details, label }: { app: TileApp; open: () => void; disabled: boolean; note?: string; details: [string, string | null | undefined][]; label?: string }) {
  const [broken, setBroken] = useState(false), [info, setInfo] = useState(false);
  const { status, reveal } = usePin();
  const lockedNow = Boolean(status?.set && !status.unlocked);
  useEffect(() => { if (lockedNow) setInfo(false); }, [lockedNow]);
  const name = appName(app);
  const rows = details.filter((row): row is [string, string] => Boolean(row[1]));
  return <div className="app-tile-wrap">
    <button className="app-tile" disabled={disabled} title={note ?? name} aria-label={label ?? `Open ${name}`} aria-description={note} onClick={open}>
      {note && <span className="app-waiting" aria-hidden />}
      {app.icon && !broken ? <img className="app-icon" src={app.icon} alt="" width={36} height={36} onError={() => setBroken(true)} /> : <span className="app-icon app-letter" aria-hidden style={{ "--hue": appHue(name) } as React.CSSProperties}>{name.charAt(0).toUpperCase()}</span>}
      <strong>{name}</strong>
    </button>
    {rows.length > 0 && <button type="button" className="app-info" aria-label={`Details for ${name}`} aria-expanded={info} title="Details" onClick={() => { if (info) setInfo(false); else void reveal().then(ok => setInfo(ok)); }}>i</button>}
    {info && <dl className="app-details">{rows.map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{value}</dd></div>)}</dl>}
  </div>;
}
