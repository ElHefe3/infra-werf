import React, { createContext, lazy, Suspense, useContext } from "react";
import type { ExtensionCatalogue, MapServer, MapState } from "./contracts.gen";
import { bind, forEnabled, type Bound } from "./extensions-model";
import { SshConnection } from "./ssh-connection";
import { FileBrowser } from "./file-browser";
import { TransferQueue, type useTransfers } from "./transfer-queue";
import { WorkloadInspectionPanel } from "./workload-inspection";
import { ElevationCheck } from "./elevation-check";
import { HealthMonitor } from "./health-monitor";
import { RemoteApps } from "./remote-apps";
import { liveStatus } from "./live-monitoring";
import { appSettings, type AppSettingsRenderer } from "./app-settings";
import { observedLabel, workloads } from "./map-model";
import { groupMenu, type MenuGroup, type MenuRenderer } from "./menu-model";
import { ServerPlugins } from "./plugins";
import { date, memory, Notice, SavedInventory } from "./workspace-parts";

// Built-in renderers for the interface extension points (S12/F18). The Rust
// core registers each contribution; this file only says how to draw it. A
// renderer keyed by an ID the core did not register is rejected by `bind`.

const TerminalPanel = lazy(() => import("./terminal-panel").then(module => ({ default: module.TerminalPanel })));

/** What a workspace tab or settings section is given to work with. */
export type WorkspaceContext = {
  server: MapServer;
  state: MapState;
  /** An SSH check or protected operation is running; controls should wait. */
  busy: boolean;
  reportBusy: (source: string, value: boolean) => void;
  refreshInventory: () => Promise<void>;
  refreshing: boolean;
  filter: string;
  setFilter: (value: string) => void;
  transfers: ReturnType<typeof useTransfers>;
  /** Switch to another tab by its contribution ID. */
  openTab: (id: string) => void;
  edit: () => void;
  /** Close this workspace and open a plugin's page in the application Settings. */
  openPlugin: (id: string) => void;
  settings: Bound<SettingsRenderer>[];
};

export type TabRenderer = { render: (context: WorkspaceContext) => React.ReactNode; footer?: string };
export type SettingsRenderer = (context: WorkspaceContext) => React.ReactNode;
export type BadgeRenderer = { slot: "heading" | "body"; render: (server: MapServer) => React.ReactNode };

/** Tabs the application shell opens by itself. Tabs are identified by ID; titles are only labels. */
export const TAB = { services: "builtin.tab.services", files: "builtin.tab.files" } as const;

export const DEFAULT_TAB_FOOTER = "Saved inventory view · Live controls require an authenticated session";

const tabs: Record<string, TabRenderer> = {
  "builtin.tab.services": { render: c => <><div className="section-heading"><div><h3>Saved services & containers</h3><p>The last approved inventory; live details and logs are fetched only when requested.</p></div><div className="inventory-refresh-actions"><input aria-label="Filter saved workloads" placeholder="Find a workload…" value={c.filter} onChange={e => c.setFilter(e.target.value)} /><button className="primary" disabled={c.busy} onClick={() => void c.refreshInventory()}>{c.refreshing ? "Refreshing…" : "Refresh inventory"}</button></div></div><SavedInventory state={{ ...c.state, servers: [c.server] }} query={c.filter} open={() => {}} single /><WorkloadInspectionPanel key={`${c.server.id}:${c.server.host}:workloads`} server={c.server} onBusy={value => c.reportBusy("workload", value)} /></> },
  "builtin.tab.health": { render: c => <HealthMonitor key={`${c.server.id}:${c.server.host}`} server={c.server} /> },
  "builtin.tab.files": { render: c => <FileBrowser key={`${c.server.id}:${c.server.host}`} server={c.server} onBusy={value => c.reportBusy("files", value)} />, footer: "File results stay in memory · Queued transfers continue until finished or locked" },
  "builtin.tab.terminal": { render: c => <Suspense fallback={<div className="unavailable"><p>Loading terminal controls…</p></div>}><TerminalPanel server={c.server} onBusy={value => c.reportBusy("terminal", value)} /></Suspense> },
  "builtin.tab.apps": { render: c => <RemoteApps key={`${c.server.id}:${c.server.host}`} server={c.server} disabled={c.busy} /> },
  "builtin.tab.system": { render: c => <><div className="section-heading"><div><h3>Saved system inventory</h3><p>Approved host details, services and containers from the latest complete collection.</p></div><button className="primary" disabled={c.busy} onClick={() => void c.refreshInventory()}>{c.refreshing ? "Refreshing…" : "Refresh inventory"}</button></div>{c.server.inventory ? <><dl className="system-grid"><div><dt>Hostname</dt><dd>{c.server.inventory.hostname}</dd></div><div><dt>Logical CPUs</dt><dd>{c.server.inventory.cpus}</dd></div><div><dt>Total memory</dt><dd>{memory(c.server.inventory.memory_total_kib)}</dd></div><div><dt>Collected on host</dt><dd>{date(c.server.inventory.collected_at)}</dd></div></dl><Notice>These are saved hardware details. Live utilisation is not collected. Open Health for separate host and service checks.</Notice></> : <Notice>No saved system inventory is available for this server.</Notice>}</> },
  "builtin.tab.settings": { render: c => <>{c.settings.map(section => <React.Fragment key={section.contribution.id}>{section.renderer(c)}</React.Fragment>)}</> },
  "builtin.tab.transfers": { render: c => <TransferQueue rows={c.transfers.rows.filter(row => row.server_id === c.server.id)} failure={c.transfers.failure} retry={() => c.openTab(TAB.files)} /> },
};

/** Renderers that belong to an optional built-in extension rather than the core (T038). */
const RENDERER_OWNERS: Record<string, string> = { "builtin.tab.apps": "builtin.remote-apps" };

const settings: Record<string, SettingsRenderer> = {
  "builtin.settings.map-details": c => <><h3>Map details</h3><dl className="system-grid"><div><dt>Name</dt><dd>{c.server.name}</dd></div><div><dt>Host</dt><dd>{c.server.host}</dd></div></dl><button className="primary" disabled={c.busy} onClick={c.edit}>Edit server details</button></>,
  "builtin.settings.ssh-connection": c => <SshConnection key={`${c.server.id}:${c.server.host}`} server={c.server} disabled={c.busy} onBusy={value => c.reportBusy("ssh", value)} />,
  "builtin.settings.elevation": c => <ElevationCheck key={`elevation:${c.server.id}:${c.server.host}`} server={c.server} disabled={c.busy} onBusy={value => c.reportBusy("elevation", value)} />,
  "builtin.settings.plugins": c => <ServerPlugins key={`plugins:${c.server.id}`} server={c.server} openPlugin={c.openPlugin} />,
};

const badges: Record<string, BadgeRenderer> = {
  "builtin.badge.observation": { slot: "heading", render: server => <span className={`observation-dot ${server.freshness}`} title={observedLabel(server)} /> },
  "builtin.badge.live-status": { slot: "body", render: server => { const live = liveStatus(server); return <p className={`live-status ${live.kind}`} title={live.detail}>{live.label}</p>; } },
  "builtin.badge.workloads": { slot: "body", render: server => {
    const preview = [...workloads(server)].sort((a, b) => Number(["active", "running"].includes(b.state)) - Number(["active", "running"].includes(a.state))).slice(0, 3);
    return <div className="node-workloads">{preview.length ? preview.map(item => <div key={item.id}><span className="workload-icon">{item.kind === "container" ? "◇" : "·"}</span><span title={`${item.scope} / ${item.name}`}>{item.name}</span><span className="small-state">{item.state}</span></div>) : <p>No saved inventory yet</p>}</div>;
  } },
};

// Main menu items (T053). The core registers each item; this says which category it sits in and which view it opens.
const menu: Record<string, MenuRenderer> = {
  "builtin.menu.topology": { category: "map", icon: "⬡", view: "topology" },
  "builtin.menu.inventory": { category: "map", icon: "▤", view: "inventory" },
  "builtin.menu.transfers": { category: "operations", icon: "⇅", view: "transfers" },
  "builtin.menu.activity": { category: "operations", icon: "◷", view: "activity" },
  "builtin.menu.servers": { category: "manage", icon: "▥", view: "servers" },
  "builtin.menu.settings": { category: "manage", icon: "⚙", view: "settings" },
};

export type BoundExtensions = { tabs: Bound<TabRenderer>[]; settings: Bound<SettingsRenderer>[]; appSettings: Bound<AppSettingsRenderer>[]; badges: Bound<BadgeRenderer>[]; menu: MenuGroup[] };

/** Pairs the core's catalogue with the built-in renderers; throws on any mismatch. */
export function bindExtensions(catalogue: ExtensionCatalogue): BoundExtensions {
  return { tabs: bind(catalogue, "workspace_tab", forEnabled(catalogue, tabs, RENDERER_OWNERS)), settings: bind(catalogue, "settings_section", settings), appSettings: bind(catalogue, "app_settings_section", appSettings), badges: bind(catalogue, "server_badge", badges), menu: groupMenu(bind(catalogue, "menu_item", menu)) };
}

export const ExtensionsContext = createContext<BoundExtensions | null>(null);
export function useExtensions(): BoundExtensions {
  const value = useContext(ExtensionsContext);
  if (!value) throw new Error("InfraWerf extensions are not loaded.");
  return value;
}

/** Server-card badges for one slot, in registry order. */
export function ServerBadges({ server, slot }: { server: MapServer; slot: BadgeRenderer["slot"] }) {
  const extensions = useContext(ExtensionsContext);
  return <>{extensions?.badges.filter(badge => badge.renderer.slot === slot).map(badge => <React.Fragment key={badge.contribution.id}>{badge.renderer.render(server)}</React.Fragment>)}</>;
}
