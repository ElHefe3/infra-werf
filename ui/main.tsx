import React, { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Background, BackgroundVariant, Handle, MarkerType, Position, ReactFlow, ReactFlowProvider, useNodesInitialized, useNodesState, useReactFlow, type Connection, type Node, type NodeProps } from "@xyflow/react";
import { createWorkspaceClient } from "./workspace-client";
import { SshConnection } from "./ssh-connection";
import { FileBrowser } from "./file-browser";
import { TransferQueue, useTransfers } from "./transfer-queue";
import { WorkloadInspectionPanel } from "./workload-inspection";
import { ElevationCheck } from "./elevation-check";
import { HealthMonitor } from "./health-monitor";
import { BrandMark } from "./brand";
import { RemoteApps } from "./remote-apps";
import { MonitoringHub, activeMap, liveStatus } from "./live-monitoring";
import type { MonitoringState } from "./contracts.gen";
import type { MapServer, MapState, Relationship, WorkspaceStatus } from "./contracts.gen";
import { errorMessage, inventoryRows, matches, observedLabel, reachabilityLabel, workloads } from "./map-model";
import "@xyflow/react/dist/style.css";
import "./style.css";

const client = createWorkspaceClient();
const TerminalPanel = lazy(() => import("./terminal-panel").then(module => ({ default: module.TerminalPanel })));
const workspaceTabs = ["Services", "Health", "Files", "Terminal", "Apps", "System", "Settings", "Transfers"] as const;
type WorkspaceTab = typeof workspaceTabs[number];
type Modal = { kind: "add" } | { kind: "edit"; id: string } | { kind: "relationship"; source: string; target: string } | { kind: "remove-server"; id: string } | { kind: "remove-edge"; edge: Relationship };
type ServerFlowNode = Node<{ server: MapServer; matched: boolean; open: (id: string) => void }, "server">;

const date = (value?: string | null) => value ? new Date(value).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }) : "No observation";
const memory = (kib: number) => `${(kib / 1024 / 1024).toLocaleString([], { maximumFractionDigits: 1 })} GiB`;

function ServerNode({ data, selected }: NodeProps<ServerFlowNode>) {
  const { server } = data;
  const live=liveStatus(server);
  const entries = workloads(server);
  const preview = [...entries].sort((a, b) => Number(["active", "running"].includes(b.state)) - Number(["active", "running"].includes(a.state))).slice(0, 3);
  return <article className={`server-card status-${live.kind} ${selected ? "is-selected" : ""} ${data.matched ? "" : "dimmed"}`} data-server-id={server.id}>
    <Handle type="target" position={Position.Left} aria-label={`Connect to ${server.name}`} />
    <div className="server-heading"><span className="server-icon">▤</span><span className={`observation-dot ${server.freshness}`} title={observedLabel(server)} /></div>
    <h3>{server.name}</h3><p className="hostname">{server.host}</p>
    <p className={`live-status ${live.kind}`} title={live.detail}>{live.label}</p>
    <div className="node-workloads">{preview.length ? preview.map(item => <div key={item.id}><span className="workload-icon">{item.kind === "container" ? "◇" : "·"}</span><span title={`${item.scope} / ${item.name}`}>{item.name}</span><span className="small-state">{item.state}</span></div>) : <p>No saved inventory yet</p>}</div>
    <footer><span>{observedLabel(server)}</span><button className="nodrag nopan icon-button" aria-label={`Open ${server.name} workspace`} onClick={e => { e.stopPropagation(); data.open(server.id); }}>↗</button></footer>
    <Handle type="source" position={Position.Right} aria-label={`Connect from ${server.name}`} />
  </article>;
}
const nodeTypes = { server: ServerNode };

function MapCanvas({ state, selected, query, busy, select, open, move, connect }: { state: MapState; selected: string | null; query: string; busy: boolean; select: (id: string | null) => void; open: (id: string) => void; move: (id: string, x: number, y: number) => Promise<boolean>; connect: (connection: Connection) => void }) {
  const [nodes, setNodes, onNodesChange] = useNodesState<ServerFlowNode>([]);
  const flow = useReactFlow();
  const measured = useNodesInitialized();
  const fitted = useRef(false);
  const dragging=useRef(false);
  useEffect(() => { if (measured && !fitted.current) { fitted.current = true; void flow.fitView({ padding: .15, maxZoom: 1 }); } }, [measured, flow]);
  useEffect(() => {if(!dragging.current) setNodes(state.servers.map(server => ({ id: server.id, type: "server", ariaLabel: server.name, position: { x: server.x, y: server.y }, selected: server.id === selected, data: { server, matched: matches(server, query), open } })));}, [state, selected, query, open, setNodes]);
  const edges = state.edges.map(edge => ({ ...edge, type: "smoothstep", markerEnd: { type: MarkerType.ArrowClosed, color: "#89a28e" }, style: { stroke: "#89a28e", strokeWidth: 1.5 }, labelStyle: { fill: "#51685a", fontSize: 11 }, labelBgStyle: { fill: "#f4f8f2" }, labelBgPadding: [7, 4] as [number, number], labelBgBorderRadius: 5 }));
  useEffect(() => {
    if (query.trim()) void flow.fitView({ nodes: state.servers.filter(s => matches(s, query)).map(s => ({ id: s.id })), padding: .25, maxZoom: 1, duration: 200 });
  }, [query, flow, state.servers]);
  return <div className="map-canvas" aria-label="Infrastructure map" onKeyDownCapture={event => {
    const offsets: Record<string, [number, number]> = { ArrowLeft: [-20, 0], ArrowRight: [20, 0], ArrowUp: [0, -20], ArrowDown: [0, 20] };
    const offset = offsets[event.key];
    const target = event.target as HTMLElement;
    const node = target.closest<HTMLElement>(".react-flow__node");
    if (!offset || !node) return;
    // Map editing is deliberately single-selection, matching its inspector.
    // Persist keyboard moves through Rust rather than React Flow's local-only
    // arrow handler. Failed/busy commands leave displayed coordinates intact.
    event.preventDefault(); event.stopPropagation();
    const server = state.servers.find(s => s.id === node.dataset.id);
    if (server && server.id === selected && target === node && !busy) void move(server.id, server.x + offset[0], server.y + offset[1]);
  }}>
    <div className="canvas-caption"><b>HOMELAB</b><span>Your infrastructure, connected</span></div>
    <ReactFlow<ServerFlowNode> nodes={nodes} edges={edges} nodeTypes={nodeTypes} onNodesChange={changes => {
      onNodesChange(changes);
      const selection = changes.find(change => change.type === "select" && change.selected);
      if (selection?.type === "select") select(selection.id);
      else if (changes.some(change => change.type === "select" && !change.selected && change.id === selected)) select(null);
    }} multiSelectionKeyCode={null} selectionKeyCode={null} selectionOnDrag={false}
      onNodeClick={(_, node) => select(node.id)} onNodeDragStart={()=>{dragging.current=true;}} onNodeDragStop={async (_, node) => { if (!await move(node.id, node.position.x, node.position.y)) setNodes(current => current.map(n => n.id === node.id ? { ...n, position: { x: n.data.server.x, y: n.data.server.y } } : n)); dragging.current=false; }}
      onConnect={connect} nodesDraggable={!busy} nodesConnectable={!busy} deleteKeyCode={null} fitView fitViewOptions={{ padding: .18, maxZoom: 1 }} minZoom={.15} maxZoom={1.8} onlyRenderVisibleElements={false}
      proOptions={{ hideAttribution: false }} ariaLabelConfig={{ "controls.zoomIn.ariaLabel": "Zoom in", "controls.zoomOut.ariaLabel": "Zoom out" }}>
      <Background variant={BackgroundVariant.Dots} gap={18} size={1} color="#d5dfcf" />
    </ReactFlow>
    {!state.servers.length && <div className="map-empty"><strong>A clear place to start.</strong><p>Add your first server to build the map.</p></div>}
    {query && !state.servers.some(s => matches(s, query)) && <div className="map-empty"><strong>No matching servers</strong><p>Try a host, service or container name.</p></div>}
    <div className="map-controls"><button aria-label="Zoom out" onClick={() => void flow.zoomOut()}>−</button><button aria-label="Zoom in" onClick={() => void flow.zoomIn()}>+</button><button onClick={() => void flow.fitView({ padding: .18, maxZoom: 1, duration: 200 })}>Fit map</button></div>
    <div className="canvas-hint">Drag to arrange · Scroll to zoom · Drag handles to connect</div>
  </div>;
}

function Dialog({ title, children, close, busy = false, wide = false }: { title: string; children: React.ReactNode; close: () => void; busy?: boolean; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { const dialog = ref.current!; dialog.showModal(); return () => dialog.close(); }, []);
  return <dialog ref={ref} className={wide ? "workspace-dialog" : "form-dialog"} aria-label={title} onCancel={event => { event.preventDefault(); if (!busy) close(); }}>
    <div className="dialog-title"><div><p className="eyebrow">INFRAWERF WORKSPACE</p><h2>{title}</h2></div><button className="icon-button" aria-label="Close dialog" disabled={busy} onClick={close}>×</button></div>
    {children}
  </dialog>;
}

function Notice({ children }: { children: React.ReactNode }) { return <div className="notice"><span aria-hidden>◇</span><div>{children}</div></div>; }

function SavedInventory({ state, query, open, single = false }: { state: MapState; query: string; open: (id: string) => void; single?: boolean }) {
  const rows = inventoryRows(state, query);
  return <div className="inventory-content"><div className="inventory-summary"><strong>{rows.length.toLocaleString()} saved workloads</strong><span>Reported state · not a live check</span></div>
    <div className="table-scroll"><table><thead><tr>{!single && <th>Server</th>}<th>Workload</th><th>Scope / engine</th><th>Reported state</th></tr></thead><tbody>
      {rows.map(({ server, workload }) => <tr key={workload.id}>{!single && <td><button className="text-button" onClick={() => open(server.id)}>{server.name}</button></td>}<td><strong>{workload.name}</strong><small>{workload.kind === "container" ? workload.detail : workload.kind}</small></td><td>{workload.scope}<small>{workload.engine}</small></td><td><span className="state-pill">{workload.state || "Unknown"}</span>{workload.kind === "service" && <small>{workload.detail}</small>}</td></tr>)}
    </tbody></table>{!rows.length && <p className="empty-copy">{query ? "No saved workloads match your search." : "No saved workloads have been imported."}</p>}</div>
  </div>;
}

function ServerWorkspace({ server, state, close, edit, reload, initialTab = "Services" }: { server: MapServer; state: MapState; close: () => void; edit: () => void; reload: () => Promise<void>; initialTab?: WorkspaceTab }) {
  const [tab, setTab] = useState<WorkspaceTab>(initialTab);
  const transfers = useTransfers();
  const [filter, setFilter] = useState("");
  const [busySources, setBusySources] = useState<Record<string, boolean>>({});
  const [refreshMessage, setRefreshMessage] = useState("");
  const [refreshFailure, setRefreshFailure] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const reportBusy = useCallback((source: string, value: boolean) => setBusySources(current => current[source] === value ? current : { ...current, [source]: value }), []);
  const checkingSsh = Object.values(busySources).some(Boolean);
  const refreshInventory = async () => {
    setRefreshing(true); setRefreshFailure(false); setRefreshMessage(""); reportBusy("inventory", true);
    try {
      const result = await client.refreshInventory(server.id);
      await reload();
      const status = result.status === "updated" ? "Approved inventory updated" : result.status === "unchanged" ? "No newer source inventory was found; the saved snapshot is unchanged" : "Collection was partial; the saved snapshot is unchanged";
      setRefreshMessage(`${status}. Source collection: ${date(result.source_collected_at)} · InfraWerf received: ${date(result.refreshed_at)}.`);
    } catch (failure) {
      setRefreshFailure(true);
      setRefreshMessage(`Inventory refresh failed. The last saved observation was retained. ${errorMessage(failure)}`);
    } finally { reportBusy("inventory", false); setRefreshing(false); }
  };
  return <Dialog title={server.name} close={close} wide busy={checkingSsh}><div className="workspace-server-line"><span>{server.host}</span><span>{observedLabel(server)} · {date(server.inventory?.collected_at)}</span></div>
    <div className="workspace-tabs" role="tablist" aria-label="Server workspace">{workspaceTabs.map(name => <button key={name} role="tab" disabled={checkingSsh} aria-selected={tab === name} onClick={() => setTab(name)}>{name}</button>)}</div>
    <section className="workspace-body" role="tabpanel" aria-label={tab}>
      {refreshMessage && <p className={`inventory-refresh-status ${refreshFailure ? "form-error" : "muted"}`} role={refreshFailure ? "alert" : "status"}>{refreshMessage}</p>}
      {tab === "Services" && <><div className="section-heading"><div><h3>Saved services & containers</h3><p>The last approved inventory; live details and logs are fetched only when requested.</p></div><div className="inventory-refresh-actions"><input aria-label="Filter saved workloads" placeholder="Find a workload…" value={filter} onChange={e => setFilter(e.target.value)} /><button className="primary" disabled={checkingSsh} onClick={() => void refreshInventory()}>{refreshing ? "Refreshing…" : "Refresh inventory"}</button></div></div><SavedInventory state={{ ...state, servers: [server] }} query={filter} open={() => {}} single /><WorkloadInspectionPanel key={`${server.id}:${server.host}:workloads`} server={server} onBusy={value => reportBusy("workload", value)} /></>}
      {tab === "Health" && <HealthMonitor key={`${server.id}:${server.host}`} server={server} />}
      {tab === "Apps" && <RemoteApps key={`${server.id}:${server.host}`} server={server} disabled={checkingSsh} />}
      {tab === "System" && <><div className="section-heading"><div><h3>Saved system inventory</h3><p>Approved host details, services and containers from the latest complete collection.</p></div><button className="primary" disabled={checkingSsh} onClick={() => void refreshInventory()}>{refreshing ? "Refreshing…" : "Refresh inventory"}</button></div>{server.inventory ? <><dl className="system-grid"><div><dt>Hostname</dt><dd>{server.inventory.hostname}</dd></div><div><dt>Logical CPUs</dt><dd>{server.inventory.cpus}</dd></div><div><dt>Total memory</dt><dd>{memory(server.inventory.memory_total_kib)}</dd></div><div><dt>Collected on host</dt><dd>{date(server.inventory.collected_at)}</dd></div></dl><Notice>These are saved hardware details. Live utilisation is not collected. Open Health for separate host and service checks.</Notice></> : <Notice>No saved system inventory is available for this server.</Notice>}</>}
      {tab === "Settings" && <><h3>Map details</h3><dl className="system-grid"><div><dt>Name</dt><dd>{server.name}</dd></div><div><dt>Host</dt><dd>{server.host}</dd></div></dl><button className="primary" disabled={checkingSsh} onClick={edit}>Edit server details</button><SshConnection key={`${server.id}:${server.host}`} server={server} disabled={checkingSsh} onBusy={value => reportBusy("ssh", value)} /><ElevationCheck key={`elevation:${server.id}:${server.host}`} server={server} disabled={checkingSsh} onBusy={value => reportBusy("elevation", value)} /></>}
      {tab === "Files" && <FileBrowser key={`${server.id}:${server.host}`} server={server} onBusy={value => reportBusy("files", value)} />}
      {tab === "Transfers" && <TransferQueue rows={transfers.rows.filter(row => row.server_id === server.id)} failure={transfers.failure} retry={() => setTab("Files")} />}
      {tab === "Terminal" && <Suspense fallback={<div className="unavailable"><p>Loading terminal controls…</p></div>}><TerminalPanel server={server} onBusy={value => reportBusy("terminal", value)} /></Suspense>}
    </section><footer className="workspace-footer">{tab === "Files" ? "File results stay in memory · Queued transfers continue until finished or locked" : "Saved inventory view · Live controls require an authenticated session"}</footer>
  </Dialog>;
}

function App() {
  const transfers = useTransfers();
  const [workspaceTab, setWorkspaceTab] = useState<WorkspaceTab>("Services");
  const [state, setState] = useState<MapState | null>(null);
  const [monitoring,setMonitoring]=useState<MonitoringState|null>(null);
  const [,setLiveClock]=useState(0);
  const [status, setStatus] = useState<WorkspaceStatus | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<"topology" | "inventory" | "activity" | "transfers" | "servers">("topology");
  const [inspector, setInspector] = useState(true);
  const [inspectorTab, setInspectorTab] = useState("Overview");
  const [modal, setModalState] = useState<Modal | null>(null);
  function setModal(value: Modal | null) { setError(""); setModalState(value); }
  const [workspace, setWorkspace] = useState<string | null>(null);
  const search = useRef<HTMLInputElement>(null);
  const refresh = useCallback(async () => {
    setError("");
    try { const value = await client.retry(); setState(value); setStatus(await client.status()); setSelected(current => current && value.servers.some(s => s.id === current) ? current : value.servers[0]?.id ?? null); }
    catch (e) { setError(errorMessage(e)); }
  }, []);
  const reloadLive=useCallback(async()=>{
    const [map,live]=await Promise.all([client.load(),client.monitoring()]);
    if (!busyRef.current) {setState(map);setMonitoring(live);}
  },[]);
  useEffect(()=>{
    if (!isTauri()) return;
    let disposed=false;let pending=false;
    async function poll(){if(pending || disposed || busyRef.current)return;pending=true;try{await reloadLive();}catch{/* Keep the last display; timestamps expose stale observations. */}finally{pending=false;}}
    void poll(); const timer=window.setInterval(()=>{setLiveClock(n=>n+1);void poll();},4000);
    return()=>{disposed=true;window.clearInterval(timer);};
  },[reloadLive]);
  useEffect(() => {
    if (isTauri()) void refresh();
    else setError("Open InfraWerf in its native Windows window to use the Rust workspace. This browser has no local data connection.");
    let disposed = false;
    let unlisten: (() => void) | undefined;
    if (isTauri()) void listen<string>("atlas-close-blocked", e => setError(e.payload)).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {});
    return () => { disposed = true; unlisten?.(); };
  }, [refresh]);
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "/" && !["INPUT", "TEXTAREA", "SELECT"].includes((e.target as HTMLElement).tagName) && !document.querySelector("dialog[open]")) { e.preventDefault(); search.current?.focus(); } };
    window.addEventListener("keydown", handler); return () => window.removeEventListener("keydown", handler);
  }, []);
  async function mutate(action: () => Promise<MapState>, message: string, close = false): Promise<boolean> {
    if (busyRef.current) return false;
    busyRef.current = true; setBusy(true); setError(""); setNotice("");
    try { const next = await action(); setState(next); setNotice(message); if (close) setModal(null); setSelected(current => current && next.servers.some(s => s.id === current) ? current : next.servers.at(-1)?.id ?? null); return true; }
    catch (e) { setError(errorMessage(e)); return false; }
    finally { busyRef.current = false; setBusy(false); }
  }
  const open = useCallback((id: string) => { setWorkspaceTab("Services"); setSelected(id); setWorkspace(id); }, []);
  const visibleState=state ? activeMap(state,monitoring) : null;
  const server = visibleState?.servers.find(s => s.id === selected);
  const workspaceServer = state?.servers.find(s => s.id === workspace);
  const related = state?.edges.filter(e => e.source === selected || e.target === selected) ?? [];
  const editServer = modal?.kind === "edit" || modal?.kind === "remove-server" ? state?.servers.find(s => s.id === modal.id) : null;
  const countWorkloads = state?.servers.reduce((n, s) => n + workloads(s).length, 0) ?? 0;
  const available = Boolean(state) && !busy;
  function relationship(source = selected ?? "", target = "") { setModal({ kind: "relationship", source, target: target || state?.servers.find(s => s.id !== source)?.id || "" }); }
  function formSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault(); const fields = new FormData(e.currentTarget); const value = (key: string) => String(fields.get(key) ?? "").trim();
    if (modal?.kind === "add") void mutate(() => client.add({ name: value("name"), host: value("host") }), "Server added to your map.", true);
    if (modal?.kind === "edit") void mutate(() => client.update(modal.id, { name: value("name"), host: value("host") }), "Server details saved.", true);
    if (modal?.kind === "relationship") void mutate(() => client.connect({ source: value("source"), target: value("target"), label: value("label") }), "Relationship documented. No server configuration changed.", true);
  }
  return <div className="app-shell">
    <aside className="rail"><BrandMark /><nav aria-label="Main navigation">{([["topology", "⬡", "Topology"], ["inventory", "▤", "Inventory"], ["transfers", "⇅", "Transfers"], ["activity", "◷", "Activity"], ["servers", "▥", "Servers & monitoring"]] as const).map(([name, icon, label]) => <button key={name} className={view === name ? "active" : ""} aria-label={label} title={label} aria-current={view === name ? "page" : undefined} onClick={() => setView(name)}>{icon}</button>)}</nav><div className="rail-bottom"><button className="icon-button quit-button" aria-label="Quit InfraWerf" title="Quit InfraWerf" onClick={() => { if (isTauri()) void getCurrentWindow().close().catch(e => setError(errorMessage(e))); }}>⏻</button><span className="local-avatar" title="Local workspace">L</span></div></aside>
    <div className="app-main"><header className="topbar"><div><strong className="wordmark">InfraWerf</strong><span>/</span> Homelab</div><span className="local-badge"><i /> LOCAL WORKSPACE</span></header>
      <main className="desktop-main"><section className="intro"><div><p className="eyebrow">YOUR INFRASTRUCTURE, CONNECTED</p><h1>A little clarity. <span>Across every server.</span></h1><p>Your infrastructure, thoughtfully mapped. One place to see how it all connects.</p></div><button className="primary" disabled={!available} onClick={() => setModal({ kind: "add" })}><span>＋</span> Add server</button></section>
        <section className="metrics" aria-label="Workspace summary"><div><span>Servers</span><strong>{state ? state.servers.length : "—"}</strong><small>{visibleState?.servers.length ?? 0} active on map</small></div><div><span>Observed workloads</span><strong>{state ? countWorkloads.toLocaleString() : "—"}</strong><small>Services & containers</small></div><div><span>Needs an observation</span><strong>{state ? state.servers.filter(s => s.freshness !== "saved").length : "—"}</strong><small>Stale or not yet observed</small></div><div><span>Saved inventories</span><strong>{state ? state.servers.filter(s => s.inventory).length : "—"}</strong><small>{visibleState?.servers.filter(s=>["offline","unreachable"].includes(liveStatus(s).kind)).length ?? 0} offline / unreachable</small></div></section>
        <div className="feedback" aria-live="polite">{error ? <div role="alert" className="error-banner"><span>{error}</span><button disabled={busy || !isTauri()} onClick={() => void refresh()}>Retry load</button><button aria-label="Dismiss error" onClick={() => setError("")}>×</button></div> : <span className="save-message">{busy ? "Saving to this PC…" : notice}</span>}</div>
        <section className={`workspace-grid ${inspector && view === "topology" ? "with-inspector" : ""}`}>
          <div className="map-panel"><div className="map-toolbar"><div className="view-tabs" aria-label="Map view"><button className={view === "topology" ? "active" : ""} onClick={() => setView("topology")}>Topology</button><button className={view === "inventory" ? "active" : ""} onClick={() => setView("inventory")}>Inventory</button></div><div className="search-box"><span aria-hidden>⌕</span><input ref={search} aria-label="Find a server or workload" placeholder="Find a server or workload…" value={query} onChange={e => setQuery(e.target.value)} />{query ? <button aria-label="Clear search" onClick={() => setQuery("")}>×</button> : <kbd>/</kbd>}</div>{view === "topology" && <button className="icon-button" aria-label="Toggle inspector" aria-pressed={inspector} onClick={() => setInspector(v => !v)}>◧</button>}</div>
            {!state ? <div className="loading-state"><BrandMark /><h2>{error ? "Your workspace couldn’t be opened." : "Opening your workspace…"}</h2><p>{error ? "Resolve the message above, then retry. Your saved data has not been replaced." : "Reading the native Rust store on this PC."}</p></div> : <>
              {view === "topology" && <ReactFlowProvider><MapCanvas state={visibleState!} selected={selected} query={query} busy={busy} select={setSelected} open={open} move={(id, x, y) => mutate(() => client.update(id, { x, y }), "Map position saved.")} connect={c => relationship(c.source, c.target)} /></ReactFlowProvider>}
              {view === "servers" && <MonitoringHub state={state} monitoring={monitoring} client={client} reload={reloadLive} remove={id=>setModal({kind:"remove-server",id})} open={open} />}
              {view === "inventory" && <SavedInventory state={state} query={query} open={open} />}
              {view === "activity" && <div className="activity-view"><h2>Workspace activity</h2><p>Timestamped reachability, inventory and lifecycle changes. Use Servers & monitoring to set history retention.</p><ActivityList state={state} /></div>}
              {view === "transfers" && <TransferQueue rows={transfers.rows} failure={transfers.failure} retry={id => { setWorkspaceTab("Files"); setSelected(id); setWorkspace(id); }} />}
            </>}
            <footer className="map-footer"><div><span><i className="legend-dot saved" /> Inventory observation</span><span><i className="legend-dot stale" /> Stale / unknown</span></div><button className="text-button" onClick={()=>setView("servers")}>Servers & monitoring</button><button className="text-button" disabled={!available || !status?.report_import_available} title={status?.report_import_available ? "Import newer approved saved reports" : "No report directory configured for this native workspace"} onClick={() => void mutate(client.importReports, "Approved report import finished.")}>↻ Import newer reports</button></footer>
          </div>
          {inspector && view === "topology" && <aside className="inspector"><div className="inspector-title"><span>INSPECTOR</span><span>⌘</span></div><div className="inspector-tabs">{["Overview", "Relationships", "Activity"].map(name => <button className={inspectorTab === name ? "active" : ""} key={name} onClick={() => setInspectorTab(name)}>{name}</button>)}</div><div className="inspector-body">{server ? <>
            <div className="selected-heading"><span className="server-icon">▤</span><span className={`observation-tag ${server.freshness}`}>{observedLabel(server)}</span></div><h2>{server.name}</h2><p className="hostname">{server.host}</p>
            {inspectorTab === "Overview" && <><p className="section-label">SAVED OBSERVATION</p><dl className="facts"><div><dt>Collected</dt><dd>{date(server.inventory?.collected_at)}</dd></div><div><dt>Logical CPUs</dt><dd>{server.inventory?.cpus ?? "Unknown"}</dd></div><div><dt>Total memory</dt><dd>{server.inventory ? memory(server.inventory.memory_total_kib) : "Unknown"}</dd></div><div><dt>Workloads</dt><dd>{workloads(server).length}</dd></div><div><dt>Reachability</dt><dd>{reachabilityLabel(server)}</dd></div></dl><Notice>Files, live metrics and controls become available through an authenticated SSH session.</Notice><div className="inspector-actions"><button disabled={busy} onClick={() => setModal({ kind: "edit", id: server.id })}>Edit details</button><button disabled={!available || state!.servers.length < 2} onClick={() => relationship()}>＋ Document relationship</button></div></>}
            {inspectorTab === "Relationships" && <><p className="section-label">DOCUMENTED CONNECTIONS</p>{related.map(edge => <div className="relationship-item" key={edge.id}><strong>{state!.servers.find(s => s.id === edge.source)?.name} → {state!.servers.find(s => s.id === edge.target)?.name}</strong><span>{edge.label || "Unlabelled"}</span><button className="text-button" disabled={busy} onClick={() => setModal({ kind: "remove-edge", edge })}>Remove relationship</button></div>)}{!related.length && <p>No relationships documented.</p>}<button disabled={!available || state!.servers.length < 2} onClick={() => relationship()}>＋ Document relationship</button><p className="muted">Diagram links describe your infrastructure. They do not change server configuration.</p></>}
            {inspectorTab === "Activity" && state && <ActivityList state={{ ...state, events: state.events.filter(e => e.server_id === server.id) }} />}
          </> : <p className="empty-copy">Select a server to inspect its saved details.</p>}</div>{server && <footer className="inspector-footer"><button className="primary" onClick={() => open(server.id)}>Open workspace <span>↗</span></button></footer>}</aside>}
        </section>
        <section className="access-strip"><span className="access-icon">⌁</span><div><strong>Your map is visible. Your access is deliberate.</strong><p>Saved inventory on this PC. Protected operations require an authenticated connection.</p></div><span className="access-status">Phone coordinator · unavailable</span></section>
        <footer className="app-footer"><b>InfraWerf</b><span>Your homelab, connected.</span></footer>
      </main>
    </div>
    {workspaceServer && state && <ServerWorkspace key={workspaceServer.id} server={workspaceServer} state={state} reload={async () => { setState(await client.retry()); }} initialTab={workspaceTab} close={() => setWorkspace(null)} edit={() => { setWorkspace(null); setModal({ kind: "edit", id: workspaceServer.id }); }} />}
    {modal && state && <Dialog title={modal.kind === "add" ? "Add a server" : modal.kind === "edit" ? "Edit server" : modal.kind === "relationship" ? "Document a relationship" : modal.kind === "remove-server" ? "Remove server from map?" : "Remove relationship?"} close={() => setModal(null)} busy={busy}>
      {error && <p role="alert" className="form-error">{error}</p>}
      {modal.kind === "add" || modal.kind === "edit" ? <form onSubmit={formSubmit}><p className="muted">A map entry stores a name and address. It does not connect to the server.</p><label>Server name<input name="name" required maxLength={80} autoFocus defaultValue={editServer?.name} placeholder="e.g. Forgejo" /></label><label>Host or IP address<input name="host" required maxLength={253} defaultValue={editServer?.host} placeholder="server.example or 192.168.1.10" spellCheck={false} /></label><div className="form-actions">{modal.kind === "edit" && <button type="button" className="danger text-button" disabled={busy} onClick={() => setModal({ kind: "remove-server", id: modal.id })}>Remove from map</button>}<button type="button" disabled={busy} onClick={() => setModal(null)}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Saving…" : "Save server"}</button></div></form>
        : modal.kind === "relationship" ? <form onSubmit={formSubmit}><p className="muted">Describe a directed connection between servers. This only updates the map.</p><label>From<select aria-label="From" name="source" defaultValue={modal.source} required>{state.servers.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label><label>To<select aria-label="To" name="target" defaultValue={modal.target} required>{state.servers.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label><label>Relationship label<input name="label" maxLength={80} placeholder="e.g. Deploys to" autoFocus /></label><div className="form-actions"><button type="button" disabled={busy} onClick={() => setModal(null)}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Saving…" : "Save relationship"}</button></div></form>
          : <><p>{modal.kind === "remove-server" ? `Permanently delete ${editServer?.name}: ${workloads(editServer!).length} saved workloads, ${state.edges.filter(e=>e.source===modal.id || e.target===modal.id).length} relationships and ${state.events.filter(e=>e.server_id===modal.id).length} retained history events? A configured alias may be rediscovered as a new candidate. Archive or ignore it in Servers to retain its history. No remote data is changed.` : `Remove the “${modal.edge.label || "unlabelled"}” diagram relationship? Server configuration will be unchanged.`}</p><div className="form-actions"><button disabled={busy} onClick={() => setModal(null)}>Cancel</button><button className="danger-button" disabled={busy} onClick={() => void mutate(() => modal.kind === "remove-server" ? client.remove(modal.id) : client.disconnect(modal.edge.id), "Map entry removed.", true)}>{busy ? "Removing…" : "Remove"}</button></div></>}
    </Dialog>}
  </div>;
}

function ActivityList({ state }: { state: MapState }) { return <ol className="activity-list">{state.events.length ? state.events.map(event => <li key={event.id}><span>{event.server_id && <strong>{state.servers.find(s=>s.id===event.server_id)?.name ?? "Removed server"}: </strong>}{event.message}</span><time dateTime={event.at}>{date(event.at)}</time></li>) : <li>No recorded activity yet.</li>}</ol>; }

createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
