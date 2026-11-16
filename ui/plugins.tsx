import React, { useCallback, useEffect, useState } from "react";
import type { Capability, CapabilityKind, ExtensionPoint, MapServer, PluginsState, PluginStatus, PluginView, TrustLevel } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";

// Plugin permissions (S12/F20/T041). The Rust core finds plugins, stores what
// the owner allowed and enforces it; these screens only show and edit that.
// Each plugin has one page, and it is the only place its permissions change.

const client = createWorkspaceClient();

export const TRUST_LEVELS: { value: TrustLevel; label: string; detail: string }[] = [
  { value: "read_only", label: "Read-only", detail: "May read and observe on the servers you allow. Every change is refused." },
  { value: "confirm_changes", label: "Changes with confirmation", detail: "May make the changes you allow, and asks you to confirm each one." },
  { value: "trusted_automation", label: "Trusted automation", detail: "May make the changes you allow without asking each time." },
];
const trustLabel = (trust: TrustLevel | null) => TRUST_LEVELS.find(level => level.value === trust)?.label ?? "";
const KIND: Record<CapabilityKind, string> = { read: "Reads", change: "Changes", elevation: "Sudo" };
const POINT: Record<ExtensionPoint, string> = {
  workspace_tab: "a tab in a server's workspace", server_badge: "a badge on a server's card", settings_section: "a section of a server's Settings tab",
  app_settings_section: "a section of this Settings screen", menu_item: "an entry in the main menu", collector: "an inventory collector",
  health_check: "a reachability check", operation: "an operation on a server",
};
export function statusLabel(status: PluginStatus, trust: TrustLevel | null): string {
  if (status === "enabled") return `Enabled · ${trustLabel(trust)}`;
  if (status === "changed_since_review") return "Changed since you reviewed it · switched off";
  return "Not enabled";
}

function PluginPage({ plugin, servers, busy, run, back }: { plugin: PluginView; servers: MapServer[]; busy: boolean; run: (action: () => Promise<PluginsState>, done: string) => Promise<void>; back: () => void }) {
  const [choice, setChoice] = useState<TrustLevel | null>(null);
  // A new review starts with nothing chosen.
  useEffect(() => setChoice(null), [plugin.id, plugin.status]);
  const grantable = plugin.capabilities.filter(entry => entry.kind !== "elevation");
  const asksSudo = plugin.capabilities.some(entry => entry.kind === "elevation");
  const allowedOn = (serverId: string): Capability[] => plugin.grants.find(grant => grant.server_id === serverId)?.capabilities ?? [];
  const setGrant = (server: MapServer, capabilities: Capability[], done: string) => run(() => client.setPluginGrant(plugin.id, server.id, capabilities), done);
  return <section className="plugin-page" aria-label={`${plugin.name} plugin`}>
    <button className="text-button" onClick={back}>← All plugins</button>
    <h4>{plugin.name} <small>version {plugin.version}</small></h4>
    {plugin.description && <p>{plugin.description}</p>}
    <dl className="plugin-facts"><div><dt>Identifier</dt><dd><code>{plugin.id}</code></dd></div>{plugin.author && <div><dt>Author</dt><dd>{plugin.author}</dd></div>}{plugin.license && <div><dt>Licence</dt><dd>{plugin.license}</dd></div>}{plugin.homepage && <div><dt>Homepage</dt><dd>{plugin.homepage}</dd></div>}<div><dt>Folder</dt><dd>{plugin.folder}</dd></div></dl>
    <p className={`plugin-status ${plugin.status}`}><strong>Status:</strong> {statusLabel(plugin.status, plugin.trust)}</p>

    <h5>What it adds</h5>
    <ul className="plugin-adds">{plugin.contributions.map(entry => <li key={entry.id}><strong>{entry.title}</strong>: {POINT[entry.point]}</li>)}</ul>

    <h5>What it asks for</h5>
    <p className="muted">This is everything the plugin can ever use. It can use each item only on the servers where you allow it below.</p>
    <ul className="plugin-asks" aria-label="Requested capabilities">{plugin.capabilities.map(entry => <li key={entry.capability}><span className={`kind ${entry.kind}`}>{KIND[entry.kind]}</span><span>{entry.summary}<small>Used by: {entry.used_by.join(", ")} · <code>{entry.capability}</code></small></span></li>)}</ul>
    {asksSudo && <p className="muted">Sudo is never part of a trust level. This plugin asks for it, and it stays refused: InfraWerf cannot yet grant sudo to a plugin.</p>}

    <h5>Trust level</h5>
    {plugin.status === "enabled" ? <>
      <p><strong>{trustLabel(plugin.trust)}.</strong> {TRUST_LEVELS.find(level => level.value === plugin.trust)?.detail}</p>
      <p className="muted">To change the trust level, disable the plugin and enable it again.</p>
      <button disabled={busy} onClick={() => void run(() => client.disablePlugin(plugin.id), `${plugin.name} disabled.`)}>Disable plugin</button>
    </> : <>
      {plugin.status === "changed_since_review" && <p className="form-error" role="alert">This plugin’s manifest changed after you enabled it, so it has been switched off. Review what it asks for above, then enable it again.</p>}
      <fieldset className="trust-choice"><legend>Choose how far this plugin may act. Nothing is chosen for you.</legend>
        {TRUST_LEVELS.map(level => <label key={level.value}><input type="radio" name={`trust-${plugin.id}`} value={level.value} checked={choice === level.value} disabled={busy} onChange={() => setChoice(level.value)} /><span><strong>{level.label}</strong><small>{level.detail}</small></span></label>)}
      </fieldset>
      <button className="primary" disabled={busy || !choice} onClick={() => { if (choice) void run(() => client.enablePlugin(plugin.id, choice), `${plugin.name} enabled: ${trustLabel(choice)}.`); }}>Enable plugin</button>
    </>}

    <h5>Servers</h5>
    <p className="muted">A plugin can do nothing on a server until you allow it here. You can set this before enabling the plugin.</p>
    {!servers.length && <p className="empty-copy">Add a server to your map first.</p>}
    {servers.map(server => { const allowed = allowedOn(server.id); return <section className="plugin-server" key={server.id} aria-label={`Permissions on ${server.name}`}>
      <header><strong>{server.name}</strong><span>{server.host}</span><span className="count">{allowed.length} of {grantable.length} allowed</span>
        <button disabled={busy || allowed.length === grantable.length} aria-label={`Allow all on ${server.name}`} onClick={() => void setGrant(server, grantable.map(entry => entry.capability), `Everything allowed on ${server.name}.`)}>Allow all</button>
        <button disabled={busy || !allowed.length} aria-label={`Withdraw all on ${server.name}`} onClick={() => void setGrant(server, [], `Everything withdrawn on ${server.name}.`)}>Withdraw all</button></header>
      {grantable.map(entry => <label key={entry.capability}><input type="checkbox" disabled={busy} checked={allowed.includes(entry.capability)} onChange={event => void setGrant(server, event.target.checked ? [...allowed, entry.capability] : allowed.filter(item => item !== entry.capability), `Permissions on ${server.name} saved.`)} /><span>{entry.summary}</span></label>)}
    </section>; })}
  </section>;
}

/** The Plugins section of the application Settings screen: the list, and one page per plugin. */
export function PluginsSection({ servers, focus, setFocus }: { servers: MapServer[]; focus: string | null; setFocus: (id: string | null) => void }) {
  const [state, setState] = useState<PluginsState | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState("");
  const [message, setMessage] = useState("");
  const run = useCallback(async (action: () => Promise<PluginsState>, done: string) => {
    setBusy(true); setFailure(""); setMessage("");
    try { setState(await action()); setMessage(done); }
    catch (error) { setFailure(errorMessage(error)); }
    finally { setBusy(false); }
  }, []);
  useEffect(() => { void run(client.plugins, ""); }, [run]);
  const selected = state?.plugins.find(plugin => plugin.id === focus) ?? null;
  return <div className="plugins">
    {failure && <p className="form-error" role="alert">{failure}</p>}{message && <p role="status">{message}</p>}
    {!state ? <p className="muted">Looking for plugins…</p> : selected ? <PluginPage plugin={selected} servers={servers} busy={busy} run={run} back={() => { setMessage(""); setFocus(null); }} /> : <>
      <p className="muted">Plugins add to InfraWerf. Finding a plugin allows nothing: you choose what each one may do, on which servers, on its own page.</p>
      {state.plugins.length ? <ul className="plugin-list" aria-label="Installed plugins">{state.plugins.map(plugin => <li key={plugin.id}><div><strong>{plugin.name}</strong> <small>version {plugin.version}</small><span className={`plugin-status ${plugin.status}`}>{statusLabel(plugin.status, plugin.trust)} · allowed on {plugin.grants.length} {plugin.grants.length === 1 ? "server" : "servers"}</span></div><button aria-label={`Configure ${plugin.name}`} onClick={() => { setMessage(""); setFocus(plugin.id); }}>Configure</button></li>)}</ul>
        : <p className="empty-copy">No plugins found.</p>}
      <p className="muted">A plugin is a folder that contains <code>infrawerf-plugin.json</code>, placed in <code>{state.directory}</code>.</p>
      <button disabled={busy} onClick={() => void run(client.plugins, "Plugin folder read again.")}>Look again</button>
      {state.invalid.length > 0 && <><h5>Folders that are not usable plugins</h5><ul className="plugin-invalid" aria-label="Unusable plugin folders">{state.invalid.map(entry => <li key={entry.folder}><strong>{entry.folder}</strong>{entry.problems.map((problem, index) => <code key={index}>{problem}</code>)}</li>)}</ul></>}
    </>}
  </div>;
}

/** Read-only list for a server's Settings tab. Permissions are changed on the plugin's page. */
export function ServerPlugins({ server, openPlugin }: { server: MapServer; openPlugin: (id: string) => void }) {
  const [state, setState] = useState<PluginsState | null>(null);
  const [failure, setFailure] = useState("");
  useEffect(() => { let current = true; client.plugins().then(value => { if (current) setState(value); }, error => { if (current) setFailure(errorMessage(error)); }); return () => { current = false; }; }, [server.id]);
  const allowed = state?.plugins.map(plugin => ({ plugin, capabilities: plugin.grants.find(grant => grant.server_id === server.id)?.capabilities ?? [] })).filter(entry => entry.capabilities.length) ?? [];
  return <section className="server-plugins" aria-label="Plugins on this server"><h3>Plugins on this server</h3>
    <p className="muted">What each plugin is allowed to use on {server.name}. Permissions are changed on the plugin’s own page in Settings.</p>
    {failure && <p className="form-error" role="alert">{failure}</p>}
    {state && !allowed.length && <p className="empty-copy">No plugin is allowed on this server.</p>}
    {allowed.length > 0 && <ul className="plugin-list">{allowed.map(({ plugin, capabilities }) => <li key={plugin.id}><div><strong>{plugin.name}</strong><span className={`plugin-status ${plugin.status}`}>{statusLabel(plugin.status, plugin.trust)}</span><small>{capabilities.join(", ")}</small></div><button aria-label={`Open ${plugin.name} plugin settings`} onClick={() => openPlugin(plugin.id)}>Open plugin settings</button></li>)}</ul>}
  </section>;
}
