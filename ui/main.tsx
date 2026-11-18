import React, { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Background, BackgroundVariant, BaseEdge, EdgeLabelRenderer, getSmoothStepPath, Handle, MarkerType, Position, ReactFlow, ReactFlowProvider, useNodesInitialized, useNodesState, useReactFlow, useUpdateNodeInternals, type Connection, type EdgeProps, type Node, type NodeProps } from "@xyflow/react";
import { createWorkspaceClient } from "./workspace-client";
import { TransferQueue, useTransfers } from "./transfer-queue";
import { BrandMark } from "./brand";
import { MonitoringHub, activeMap, liveStatus } from "./live-monitoring";
import { bindExtensions, DEFAULT_TAB_FOOTER, ExtensionsContext, PackMapContext, ServerBadges, TAB, useExtensions, type BoundExtensions, type WorkspaceContext } from "./extensions";
import { date, memory, Notice, SavedInventory } from "./workspace-parts";
import { SettingsScreen } from "./app-settings";
import { Dialog } from "./dialog";
import { PluginActionDialog, PluginViewPanel, usePluginViews, viewTabId } from "./plugin-view";
import { JobAskDialog } from "./plugin-jobs";
import { MenuSystem, useMenu } from "./menu";
import type { View } from "./menu-model";
import type { MonitoringState, PackMapState } from "./contracts.gen";
import { edgeLook, kindKey, kindsBetween } from "./pack-map-model";
import type { MapServer, MapState, Relationship, WorkspaceStatus } from "./contracts.gen";
import { CARD, errorMessage, labelSize, matches, observedLabel, pathPoints, placeLabels, reachabilityLabel, routeEdges, workloads, type Point, type Port, type Side } from "./map-model";
import "@xyflow/react/dist/style.css";
import "./style.css";
import "./liquid-glass.css";
import "./dark.css";
import { useLook } from "./look";
import { LockButton, PinProvider } from "./pin";

const client = createWorkspaceClient();
// A workspace tab is identified by its contribution ID (for example TAB.files).
type WorkspaceTab = string;
type Modal = { kind: "add" } | { kind: "edit"; id: string } | { kind: "relationship"; source: string; target: string } | { kind: "remove-server"; id: string } | { kind: "remove-edge"; edge: Relationship };
type ServerFlowNode = Node<{ server: MapServer; matched: boolean; open: (id: string) => void; ports: Port[] }, "server">;
const SIDE: Record<Side, Position> = { top: Position.Top, right: Position.Right, bottom: Position.Bottom, left: Position.Left };


function ServerNode({ data, selected }: NodeProps<ServerFlowNode>) {
  const { server } = data;
  const live=liveStatus(server);
  return <article className={`server-card status-${live.kind} ${selected ? "is-selected" : ""} ${data.matched ? "" : "dimmed"}`} data-server-id={server.id}>
    <Handle type="target" id="connect-in" position={Position.Left} aria-label={`Connect to ${server.name}`} />
    {data.ports.map(port => <Handle key={port.id} id={port.id} type={port.type} position={SIDE[port.side]} isConnectable={false} className="route-handle" style={port.side === "left" || port.side === "right" ? { top: `${port.offset * 100}%` } : { left: `${port.offset * 100}%` }} />)}
    <div className="server-heading"><span className="server-icon">▤</span><ServerBadges server={server} slot="heading" /></div>
    <h3>{server.name}</h3><p className="hostname">{server.host}</p>
    <ServerBadges server={server} slot="body" />
    <footer><span>{observedLabel(server)}</span><button className="nodrag nopan icon-button" aria-label={`Open ${server.name} workspace`} onClick={e => { e.stopPropagation(); data.open(server.id); }}>↗</button></footer>
    <Handle type="source" id="connect-out" position={Position.Right} aria-label={`Connect from ${server.name}`} />
  </article>;
}
const nodeTypes = { server: ServerNode };

// Relationship lines draw their own label so the map can place every label
// where it covers no other label and no card (T063).
type LabelReport = { points: Point[]; width: number; height: number };
const Labels = createContext<{ report: (id: string, value: LabelReport | null) => void; places: Map<string, Point> }>({ report: () => {}, places: new Map() });
function RoutedEdge({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, markerEnd, style, label }: EdgeProps) {
  const [path, labelX, labelY] = getSmoothStepPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition, borderRadius: 10, offset: 18 });
  const { report, places } = useContext(Labels);
  const text = typeof label === "string" ? label : "";
  useLayoutEffect(() => { if (text) report(id, { points: pathPoints(path), ...labelSize(text) }); return () => report(id, null); }, [id, path, text, report]);
  const at = places.get(id) ?? { x: labelX, y: labelY };
  return <><BaseEdge id={id} path={path} markerEnd={markerEnd} style={style} />{text && <EdgeLabelRenderer><div className="edge-label" style={{ transform: `translate(-50%, -50%) translate(${at.x}px, ${at.y}px)` }}>{text}</div></EdgeLabelRenderer>}</>;
}
const edgeTypes = { routed: RoutedEdge };

function MapCanvas({ state, packs, selected, query, busy, select, open, move, connect }: { state: MapState; packs: PackMapState | null; selected: string | null; query: string; busy: boolean; select: (id: string | null) => void; open: (id: string) => void; move: (id: string, x: number, y: number) => Promise<boolean>; connect: (connection: Connection) => void }) {
  const [nodes, setNodes, onNodesChange] = useNodesState<ServerFlowNode>([]);
  const flow = useReactFlow();
  const measured = useNodesInitialized();
  const fitted = useRef(false);
  const dragging=useRef(false);
  useEffect(() => { if (measured && !fitted.current) { fitted.current = true; void flow.fitView({ padding: .15, maxZoom: 1 }); } }, [measured, flow]);
  // Each relationship gets its own connection points (T063); recomputed when servers move.
  const routing = useMemo(() => routeEdges(state.servers, state.edges), [state.servers, state.edges]);
  const updateInternals = useUpdateNodeInternals();
  useEffect(() => {if(!dragging.current) setNodes(state.servers.map(server => ({ id: server.id, type: "server", ariaLabel: server.name, position: { x: server.x, y: server.y }, selected: server.id === selected, data: { server, matched: matches(server, query), open, ports: routing.ports.get(server.id) ?? [] } })));}, [state, selected, query, open, setNodes, routing]);
  useEffect(() => { updateInternals(state.servers.map(server => server.id)); }, [routing, updateInternals, state.servers]);
  const edges = state.edges.map(edge => { const look = edgeLook(edge, packs?.relationship_kinds ?? []); const route = routing.routes.get(edge.id); return { ...edge, ...(route ?? { sourceHandle: "connect-out", targetHandle: "connect-in" }), label: look.label, type: "routed", markerEnd: { type: MarkerType.ArrowClosed, color: look.color }, style: { stroke: look.color, strokeWidth: 1.5, strokeDasharray: look.dash } }; });
  // Label placement: each line reports its path; one pass places them all.
  const reports = useRef(new Map<string, LabelReport>());
  const [reported, setReported] = useState(0);
  const pending = useRef(0);
  const report = useCallback((id: string, value: LabelReport | null) => {
    if (value) reports.current.set(id, value); else reports.current.delete(id);
    if (!pending.current) pending.current = requestAnimationFrame(() => { pending.current = 0; setReported(n => n + 1); });
  }, []);
  useEffect(() => () => cancelAnimationFrame(pending.current), []);
  const places = useMemo(() => placeLabels(
    [...reports.current].sort(([a], [b]) => a < b ? -1 : 1).map(([id, value]) => ({ id, ...value })),
    nodes.map(node => ({ x: node.position.x, y: node.position.y, width: node.measured?.width ?? CARD.width, height: node.measured?.height ?? CARD.height })),
  // eslint-disable-next-line react-hooks/exhaustive-deps
  ), [reported, nodes]);
  const labels = useMemo(() => ({ report, places }), [report, places]);
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
        <Labels.Provider value={labels}><ReactFlow<ServerFlowNode> nodes={nodes} edges={edges} nodeTypes={nodeTypes} edgeTypes={edgeTypes} onNodesChange={changes => {
      onNodesChange(changes);
      const selection = changes.find(change => change.type === "select" && change.selected);
      if (selection?.type === "select") select(selection.id);
      else if (changes.some(change => change.type === "select" && !change.selected && change.id === selected)) select(null);
    }} multiSelectionKeyCode={null} selectionKeyCode={null} selectionOnDrag={false}
      onNodeClick={(_, node) => select(node.id)} onNodeDragStart={()=>{dragging.current=true;}} onNodeDragStop={async (_, node) => { if (!await move(node.id, node.position.x, node.position.y)) setNodes(current => current.map(n => n.id === node.id ? { ...n, position: { x: n.data.server.x, y: n.data.server.y } } : n)); dragging.current=false; }}
      onConnect={connect} nodesDraggable={!busy} nodesConnectable={!busy} deleteKeyCode={null} fitView fitViewOptions={{ padding: .18, maxZoom: 1 }} minZoom={.15} maxZoom={1.8} onlyRenderVisibleElements={false}
      proOptions={{ hideAttribution: false }} ariaLabelConfig={{ "controls.zoomIn.ariaLabel": "Zoom in", "controls.zoomOut.ariaLabel": "Zoom out" }}>
      <Background variant={BackgroundVariant.Dots} gap={18} size={1} color="#d5dfcf" />
    </ReactFlow></Labels.Provider>
    {!state.servers.length && <div className="map-empty"><strong>A clear place to start.</strong><p>Add your first server to build the map.</p></div>}
    {query && !state.servers.some(s => matches(s, query)) && <div className="map-empty"><strong>No matching servers</strong><p>Try a host, service or container name.</p></div>}
    <div className="map-controls"><button aria-label="Zoom out" onClick={() => void flow.zoomOut()}>−</button><button aria-label="Zoom in" onClick={() => void flow.zoomIn()}>+</button><button onClick={() => void flow.fitView({ padding: .18, maxZoom: 1, duration: 200 })}>Fit map</button></div>
    <div className="canvas-hint"><span><i className="legend-dot saved" /> Saved observation</span><span><i className="legend-dot stale" /> Stale / unknown</span><span className="hint-help">Drag to arrange · Scroll to zoom · Drag handles to connect</span></div>
  </div>;
}



function ServerWorkspace({ server, state, close, edit, reload, openPlugin, initialTab = TAB.services }: { server: MapServer; state: MapState; close: () => void; edit: () => void; reload: () => Promise<void>; openPlugin: (id: string) => void; initialTab?: WorkspaceTab }) {
  const extensions = useExtensions();
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
  const views = usePluginViews(server.id);
  const activeView = views.find(view => viewTabId(view) === tab);
  const active = activeView ? undefined : extensions.tabs.find(entry => entry.contribution.id === tab) ?? extensions.tabs[0];
  const context: WorkspaceContext = { server, state, busy: checkingSsh, reportBusy, refreshInventory, refreshing, filter, setFilter, transfers, openTab: setTab, edit, openPlugin, settings: extensions.settings };
  return <Dialog title={server.name} close={close} wide busy={checkingSsh}><div className="workspace-server-line"><span>{server.host}</span><span>{observedLabel(server)} · {date(server.inventory?.collected_at)}</span></div>
    <div className="workspace-tabs" role="tablist" aria-label="Server workspace">{extensions.tabs.map(({ contribution }) => <button key={contribution.id} role="tab" disabled={checkingSsh} aria-selected={active?.contribution.id === contribution.id} onClick={() => setTab(contribution.id)}>{contribution.title}</button>)}
      {views.map(view => <button key={viewTabId(view)} role="tab" className="plugin-tab" disabled={checkingSsh} aria-selected={activeView === view} title={`From the plugin ${view.plugin_name}`} onClick={() => setTab(viewTabId(view))}>{view.title}</button>)}</div>
    <section className="workspace-body" role="tabpanel" aria-label={activeView?.title ?? active?.contribution.title}>
      {refreshMessage && <p className={`inventory-refresh-status ${refreshFailure ? "form-error" : "muted"}`} role={refreshFailure ? "alert" : "status"}>{refreshMessage}</p>}
      {activeView && <PluginViewPanel key={viewTabId(activeView)} server={server} view={activeView} />}
      {active && <React.Fragment key={active.contribution.id}>{active.renderer.render(context)}</React.Fragment>}
    </section><footer className="workspace-footer">{activeView ? "Plugin view · its own page, isolated from InfraWerf · closes with this workspace" : active?.renderer.footer ?? DEFAULT_TAB_FOOTER}</footer>
  </Dialog>;
}

function App() {
  const transfers = useTransfers();
  const [workspaceTab, setWorkspaceTab] = useState<WorkspaceTab>(TAB.services);
  const [state, setState] = useState<MapState | null>(null);
  const [extensions, setExtensions] = useState<BoundExtensions | null>(null);
  // The map is shown only once the extension catalogue is bound; a card
  // without its badges, or a workspace that cannot open, is worse than none.
  const extensionsReady = useRef(false);
  const [monitoring,setMonitoring]=useState<MonitoringState|null>(null);
  const [packs, setPacks] = useState<PackMapState | null>(null);
  const [,setLiveClock]=useState(0);
  const [status, setStatus] = useState<WorkspaceStatus | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  // A confirmation fades after a moment; errors stay until dismissed.
  useEffect(() => { if (!notice) return; const timer = window.setTimeout(() => setNotice(""), 3000); return () => window.clearTimeout(timer); }, [notice]);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<View>("topology");
  const menu = useMenu();
  const { look, theme, toggleLook, toggleTheme } = useLook();
  const [pluginFocus, setPluginFocus] = useState<string | null>(null);
  const [inspector, setInspector] = useState(true);
  const [inspectorTab, setInspectorTab] = useState("Overview");
  const [modal, setModalState] = useState<Modal | null>(null);
  function setModal(value: Modal | null) { setError(""); setModalState(value); }
  const [workspace, setWorkspace] = useState<string | null>(null);
  const search = useRef<HTMLInputElement>(null);
  const refresh = useCallback(async () => {
    setError("");
    try { const value = await client.retry(); const bound = bindExtensions(await client.extensions()); const live = await client.monitoring(); setPacks(await client.packMap().catch(() => null)); setExtensions(bound); extensionsReady.current = true; setMonitoring(live); setState(value); setStatus(await client.status()); setSelected(current => current && value.servers.some(s => s.id === current) ? current : value.servers[0]?.id ?? null); }
    catch (e) { setError(errorMessage(e)); }
  }, []);
  const reloadLive=useCallback(async()=>{
    const [map,live,pack]=await Promise.all([client.load(),client.monitoring(),client.packMap().catch(() => null)]);
    if (!busyRef.current && extensionsReady.current) {setState(map);setMonitoring(live);setPacks(pack);}
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
    try { const next = await action(); const live = await client.monitoring().catch(() => null); setState(next); if (live) setMonitoring(live); setNotice(message); if (close) setModal(null); setSelected(current => current && next.servers.some(s => s.id === current) ? current : next.servers.at(-1)?.id ?? null); return true; }
    catch (e) { setError(errorMessage(e)); return false; }
    finally { busyRef.current = false; setBusy(false); }
  }
  const open = useCallback((id: string) => { setWorkspaceTab(TAB.services); setSelected(id); setWorkspace(id); }, []);
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
    if (modal?.kind === "relationship") { const [plugin_id, contribution_id] = value("kind").split("/"); void mutate(() => client.connect({ source: value("source"), target: value("target"), label: value("label"), ...(plugin_id && contribution_id ? { kind: { plugin_id, contribution_id } } : {}) }), "Relationship documented. No server configuration changed.", true); }
  }
  return <ExtensionsContext.Provider value={extensions}><PackMapContext.Provider value={packs}><div className={`app-shell ${menu.layout.open ? "" : "menu-hidden"}`}>
    <MenuSystem groups={extensions?.menu ?? []} view={view} go={setView} menu={menu} quit={() => { if (isTauri()) void getCurrentWindow().close().catch(e => setError(errorMessage(e))); }} />
    <div className="app-main"><header className="topbar"><div className="topbar-title"><strong className="wordmark">InfraWerf</strong><span>/</span> Homelab</div><dl className="topbar-stats" aria-label="Workspace summary"><div title={`${visibleState?.servers.length ?? 0} active on the map`}><dt>Servers</dt><dd>{state ? state.servers.length : "—"}</dd></div><div title="Saved services and containers"><dt>Workloads</dt><dd>{state ? countWorkloads.toLocaleString() : "—"}</dd></div><div title="Stale or not yet observed"><dt>Need observation</dt><dd>{state ? state.servers.filter(s => s.freshness !== "saved").length : "—"}</dd></div><div title={`Servers with a saved inventory · ${visibleState?.servers.filter(s=>["offline","unreachable"].includes(liveStatus(s).kind)).length ?? 0} offline or unreachable`}><dt>Inventories</dt><dd>{state ? state.servers.filter(s => s.inventory).length : "—"}</dd></div></dl><div className="topbar-end"><button className="primary add-server" disabled={!available} onClick={() => setModal({ kind: "add" })}><span>＋</span> Add server</button><button className="look-switch" onClick={toggleLook}>{look === "glass" ? "◑ Classic look" : "◐ Glass look"}</button><button className="look-switch" onClick={toggleTheme}>{theme === "dark" ? "☀ Light mode" : "☾ Dark mode"}</button><LockButton /><span className="local-badge" title="Saved inventory stays on this PC. Protected operations require an authenticated connection. Phone coordinator: unavailable."><i /> Local</span></div></header>
      <main className="desktop-main">
        <div className={`feedback ${error || busy || notice ? "shown" : ""}`} aria-live="polite">{error ? <div role="alert" className="error-banner"><span>{error}</span><button disabled={busy || !isTauri()} onClick={() => void refresh()}>Retry load</button><button aria-label="Dismiss error" onClick={() => setError("")}>×</button></div> : <span className="save-message">{busy ? "Saving to this PC…" : notice}</span>}</div>
        {!inspector && view === "topology" && <button className="panel-toggle inspector-open" aria-label="Show inspector" title="Show inspector" aria-expanded="false" aria-controls="inspector" onClick={() => setInspector(true)}>◨</button>}
        <section className={`workspace-grid ${inspector && view === "topology" ? "with-inspector" : ""}`}>
          <div className="map-panel"><div className="map-toolbar"><div className="view-tabs" aria-label="Map view"><button className={view === "topology" ? "active" : ""} onClick={() => setView("topology")}>Topology</button><button className={view === "inventory" ? "active" : ""} onClick={() => setView("inventory")}>Inventory</button></div><div className="search-box"><span aria-hidden>⌕</span><input ref={search} aria-label="Find a server or workload" placeholder="Find a server or workload…" value={query} onChange={e => setQuery(e.target.value)} />{query ? <button aria-label="Clear search" onClick={() => setQuery("")}>×</button> : <kbd>/</kbd>}</div>{status?.report_import_available && <button className="text-button" disabled={!available} title="Import newer approved saved reports" onClick={() => void mutate(client.importReports, "Approved report import finished.")}>↻ Import reports</button>}</div>
            {!state ? <div className="loading-state"><BrandMark /><h2>{error ? "Your workspace couldn’t be opened." : "Opening your workspace…"}</h2><p>{error ? "Resolve the message above, then retry. Your saved data has not been replaced." : "Reading the native Rust store on this PC."}</p></div> : <>
              {view === "topology" && <ReactFlowProvider><MapCanvas state={visibleState!} packs={packs} selected={selected} query={query} busy={busy} select={setSelected} open={open} move={(id, x, y) => mutate(() => client.update(id, { x, y }), "Map position saved.")} connect={c => relationship(c.source, c.target)} /></ReactFlowProvider>}
              {view === "servers" && <MonitoringHub state={state} monitoring={monitoring} client={client} reload={reloadLive} remove={id=>setModal({kind:"remove-server",id})} open={open} openSettings={()=>setView("settings")} />}
              {view === "settings" && extensions && <SettingsScreen sections={extensions.appSettings} monitoring={monitoring} client={client} reload={reloadLive} resetMenu={menu.reset} servers={state.servers} pluginFocus={pluginFocus} setPluginFocus={setPluginFocus} />}
              {view === "inventory" && <SavedInventory state={state} query={query} open={open} />}
              {view === "activity" && <div className="activity-view"><h2>Workspace activity</h2><p>Timestamped reachability, inventory and lifecycle changes. Use Settings to set history retention.</p><ActivityList state={state} /></div>}
              {view === "transfers" && <TransferQueue rows={transfers.rows} failure={transfers.failure} retry={id => { setWorkspaceTab(TAB.files); setSelected(id); setWorkspace(id); }} />}
            </>}
            
          </div>
          {inspector && view === "topology" && <aside className="inspector" id="inspector"><button className="panel-toggle inspector-close" aria-label="Hide inspector" title="Hide inspector" aria-expanded="true" aria-controls="inspector" onClick={() => setInspector(false)}>◨</button><div className="inspector-tabs">{["Overview", "Relationships", "Activity"].map(name => <button className={inspectorTab === name ? "active" : ""} key={name} onClick={() => setInspectorTab(name)}>{name}</button>)}</div><div className="inspector-body">{server ? <>
            <div className="selected-heading"><span className="server-icon">▤</span><span className={`observation-tag ${server.freshness}`}>{observedLabel(server)}</span></div><h2>{server.name}</h2><p className="hostname">{server.host}</p>
            {inspectorTab === "Overview" && <><p className="section-label">SAVED OBSERVATION</p><dl className="facts"><div><dt>Collected</dt><dd>{date(server.inventory?.collected_at)}</dd></div><div><dt>Logical CPUs</dt><dd>{server.inventory?.cpus ?? "Unknown"}</dd></div><div><dt>Total memory</dt><dd>{server.inventory ? memory(server.inventory.memory_total_kib) : "Unknown"}</dd></div><div><dt>Workloads</dt><dd>{workloads(server).length}</dd></div><div><dt>Reachability</dt><dd>{reachabilityLabel(server)}</dd></div></dl><Notice>Files, live metrics and controls become available through an authenticated SSH session.</Notice><div className="inspector-actions"><button disabled={busy} onClick={() => setModal({ kind: "edit", id: server.id })}>Edit details</button><button disabled={!available || state!.servers.length < 2} onClick={() => relationship()}>＋ Document relationship</button></div></>}
            {inspectorTab === "Relationships" && <><p className="section-label">DOCUMENTED CONNECTIONS</p>{related.map(edge => <div className="relationship-item" key={edge.id}><strong>{state!.servers.find(s => s.id === edge.source)?.name} → {state!.servers.find(s => s.id === edge.target)?.name}</strong><span>{edgeLook(edge, packs?.relationship_kinds ?? []).label || "Unlabelled"}</span><button className="text-button" disabled={busy} onClick={() => setModal({ kind: "remove-edge", edge })}>Remove relationship</button></div>)}{!related.length && <p>No relationships documented.</p>}<button disabled={!available || state!.servers.length < 2} onClick={() => relationship()}>＋ Document relationship</button><p className="muted">Diagram links describe your infrastructure. They do not change server configuration.</p></>}
            {inspectorTab === "Activity" && state && <ActivityList state={{ ...state, events: state.events.filter(e => e.server_id === server.id) }} />}
          </> : <p className="empty-copy">Select a server to inspect its saved details.</p>}</div>{server && <footer className="inspector-footer"><button className="primary" onClick={() => open(server.id)}>Open workspace <span>↗</span></button></footer>}</aside>}
        </section>
        
      </main>
    </div>
    {workspaceServer && state && extensions && <ServerWorkspace key={workspaceServer.id} server={workspaceServer} state={state} openPlugin={id => { setWorkspace(null); setPluginFocus(id); setView("settings"); }} reload={async () => { setState(await client.retry()); }} initialTab={workspaceTab} close={() => setWorkspace(null)} edit={() => { setWorkspace(null); setModal({ kind: "edit", id: workspaceServer.id }); }} />}
    {modal && state && <Dialog title={modal.kind === "add" ? "Add a server" : modal.kind === "edit" ? "Edit server" : modal.kind === "relationship" ? "Document a relationship" : modal.kind === "remove-server" ? "Remove server from map?" : "Remove relationship?"} close={() => setModal(null)} busy={busy}>
      {error && <p role="alert" className="form-error">{error}</p>}
      {modal.kind === "add" || modal.kind === "edit" ? <form onSubmit={formSubmit}><p className="muted">A map entry stores a name and address. It does not connect to the server.</p><label>Server name<input name="name" required maxLength={80} autoFocus defaultValue={editServer?.name} placeholder="e.g. Forgejo" /></label><label>Host or IP address<input name="host" required maxLength={253} defaultValue={editServer?.host} placeholder="server.example or 192.168.1.10" spellCheck={false} /></label><div className="form-actions">{modal.kind === "edit" && <button type="button" className="danger text-button" disabled={busy} onClick={() => setModal({ kind: "remove-server", id: modal.id })}>Remove from map</button>}<button type="button" disabled={busy} onClick={() => setModal(null)}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Saving…" : "Save server"}</button></div></form>
        : modal.kind === "relationship" ? <form onSubmit={formSubmit}><p className="muted">Describe a directed connection between servers. This only updates the map.</p><label>From<select aria-label="From" name="source" defaultValue={modal.source} required onChange={e => setModalState({ ...modal, source: e.target.value })}>{state.servers.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label><label>To<select aria-label="To" name="target" defaultValue={modal.target} required onChange={e => setModalState({ ...modal, target: e.target.value })}>{state.servers.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label><RelationshipKindField kinds={kindsBetween(packs?.relationship_kinds ?? [], modal.source, modal.target)} /><label>Relationship label<input name="label" maxLength={80} placeholder="e.g. Deploys to" autoFocus /></label><div className="form-actions"><button type="button" disabled={busy} onClick={() => setModal(null)}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Saving…" : "Save relationship"}</button></div></form>
          : <><p>{modal.kind === "remove-server" ? `Permanently delete ${editServer?.name}: ${workloads(editServer!).length} saved workloads, ${state.edges.filter(e=>e.source===modal.id || e.target===modal.id).length} relationships and ${state.events.filter(e=>e.server_id===modal.id).length} retained history events? A configured alias may be rediscovered as a new candidate. Archive or ignore it in Servers to retain its history. No remote data is changed.` : `Remove the “${modal.edge.label || "unlabelled"}” diagram relationship? Server configuration will be unchanged.`}</p><div className="form-actions"><button disabled={busy} onClick={() => setModal(null)}>Cancel</button><button className="danger-button" disabled={busy} onClick={() => void mutate(() => modal.kind === "remove-server" ? client.remove(modal.id) : client.disconnect(modal.edge.id), "Map entry removed.", true)}>{busy ? "Removing…" : "Remove"}</button></div></>}
    </Dialog>}
    <PluginActionDialog />
    <JobAskDialog />
  </div></PackMapContext.Provider></ExtensionsContext.Provider>;
}

/** The kinds plugins offer for this pair of servers; choosing one only changes how the map shows it. */
function RelationshipKindField({ kinds }: { kinds: PackMapState["relationship_kinds"] }) {
  if (!kinds.length) return null;
  return <label>Kind<select aria-label="Relationship kind" name="kind" defaultValue=""><option value="">Plain relationship</option>{kinds.map(kind => <option key={kindKey(kind)} value={kindKey(kind)}>{kind.title} · from {kind.plugin_name}</option>)}</select><small className="muted">A kind is a label and a line style on your map. It applies nothing.</small></label>;
}

function ActivityList({ state }: { state: MapState }) { return <ol className="activity-list">{state.events.length ? state.events.map(event => <li key={event.id}><span>{event.server_id && <strong>{state.servers.find(s=>s.id===event.server_id)?.name ?? "Removed server"}: </strong>}{event.message}</span><time dateTime={event.at}>{date(event.at)}</time></li>) : <li>No recorded activity yet.</li>}</ol>; }

createRoot(document.getElementById("root")!).render(<React.StrictMode><PinProvider><App /></PinProvider></React.StrictMode>);
