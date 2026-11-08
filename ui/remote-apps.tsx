import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { MapServer, TunnelInfo } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { type AppShortcut, loadShortcuts, parseTarget, saveShortcuts } from "./remote-apps-model";
const client = createWorkspaceClient();

/** F17/T035: open a server's localhost web interfaces in isolated windows. */
export function RemoteApps({ server, disabled }: { server: MapServer; disabled: boolean }) {
  const [tunnels, setTunnels] = useState<TunnelInfo[]>([]);
  const [shortcuts, setShortcuts] = useState<AppShortcut[]>(() => loadShortcuts(server.id));
  const [port, setPort] = useState(""), [path, setPath] = useState("/"), [name, setName] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [notice, setNotice] = useState("");
  async function refresh() { try { setTunnels((await client.tunnels()).filter(t => t.server_id === server.id)); } catch (e) { setError(errorMessage(e)); } }
  useEffect(() => {
    let disposed = false, unlisten: (() => void) | undefined;
    void refresh();
    void listen("atlas-tunnels-changed", () => void refresh()).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {});
    const timer = setInterval(() => void refresh(), 5000);
    return () => { disposed = true; clearInterval(timer); unlisten?.(); };
  }, [server.id]);
  async function open(target: { port: number; path: string; name?: string }) {
    setBusy(true); setError(""); setNotice("");
    try {
      const info = await client.openTunnel(server.id, { remote_port: target.port, path: target.path });
      setNotice(`${target.name || `localhost:${target.port}`} opened in its own window through a private tunnel.`);
      setTunnels(current => [...current.filter(t => t.tunnel_id !== info.tunnel_id), info]);
    } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
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
  return <div className="remote-apps">
    <div className="section-heading"><div><h3>Remote applications</h3><p>Open web interfaces that listen only on {server.name}'s localhost, in their own InfraWerf window.</p></div>
      {tunnels.length > 0 && <button className="danger text-button" onClick={() => void client.closeAllTunnels().then(refresh).catch(e => setError(errorMessage(e)))}>Close all tunnels</button>}</div>
    {error && <p className="form-error" role="alert">{error}</p>}{notice && <p role="status" className="apps-notice">{notice}</p>}
    {shortcuts.length > 0 && <><p className="section-label">SAVED APPLICATIONS</p><div className="app-shortcuts">{shortcuts.map(item => <div className="app-shortcut" key={item.name}>
      <button className="app-launch" disabled={locked} onClick={() => void open(item)}><strong>{item.name}</strong><span>localhost:{item.port}{item.path !== "/" ? item.path : ""}</span></button>
      <button className="text-button" aria-label={`Forget ${item.name}`} onClick={() => forget(item)}>Forget</button></div>)}</div></>}
    <form className="app-form" onSubmit={submit}>
      <label>Remote port<input aria-label="Remote port" inputMode="numeric" required placeholder="e.g. 3000" value={port} onChange={e => setPort(e.target.value)} /></label>
      <label>Path<input aria-label="Path" value={path} spellCheck={false} onChange={e => setPath(e.target.value)} /></label>
      <label>Name (for a shortcut)<input aria-label="Application name" maxLength={60} placeholder="e.g. Grafana" value={name} onChange={e => setName(e.target.value)} /></label>
      <div className="app-form-actions"><button type="button" disabled={locked} onClick={remember}>Save shortcut</button><button className="primary" disabled={locked}>{busy ? "Verifying and connecting…" : "Open application"}</button></div>
    </form>
    <p className="section-label">OPEN TUNNELS</p>
    {tunnels.length ? <ul className="tunnel-list">{tunnels.map(t => <li key={t.tunnel_id}><div><strong>localhost:{t.remote_port}</strong><span>via 127.0.0.1:{t.local_port} · opened {new Date(t.opened_at).toLocaleTimeString()} · closes by {new Date(t.expires_at).toLocaleTimeString()}</span></div>
      <div className="tunnel-actions"><button onClick={() => void client.focusTunnel(t.tunnel_id).catch(e => setError(errorMessage(e)))}>Show window</button><button className="danger text-button" onClick={() => void close(t.tunnel_id)}>Close</button></div></li>)}</ul>
      : <p className="muted">No tunnels are open for this server.</p>}
    <p className="muted">InfraWerf first verifies SSH access with your existing OpenSSH trust, then forwards a random port on this PC (127.0.0.1 only) to localhost on the server. Nothing is installed or exposed on your network. Application windows run privately, cannot control InfraWerf, and stay on the tunnel address. Closing the window, archiving or pausing the server, or quitting InfraWerf ends the tunnel; tunnels also close after four hours.</p>
  </div>;
}
