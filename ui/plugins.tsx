import React, { useCallback, useEffect, useState } from "react";
import type { Capability, CapabilityKind, ExtensionPoint, MapServer, PluginActivity, PluginActivityLog, PluginsState, PluginStatus, PluginView, Refusal, TrustLevel } from "./contracts.gen";
import { date } from "./workspace-parts";
import { commandLine } from "./plugin-data";
import type { PackCommand } from "./contracts.gen";
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
  remote_app: "an application in a server's Apps list", relationship_kind: "a kind of relationship for your map",
};
const REFUSAL: Record<Refusal, string> = {
  unknown_plugin: "the plugin is not installed", plugin_disabled: "the plugin is not enabled", changed_since_review: "the plugin changed after you reviewed it",
  unknown_server: "the server is not on your map", not_requested: "the plugin never asked for it", not_granted: "it is not allowed on this server",
  trust_too_low: "the plugin is read-only", no_elevation_exception: "sudo cannot be given to a plugin", all_plugins_off: "all plugins are turned off",
};
const OWNER_ACTION: Record<string, string> = {
  enable: "Enabled by you", disable: "Disabled by you", set_permissions: "Permissions set by you", revoke_trusted_automation: "Trusted automation revoked by you",
  all_plugins_off: "All plugins turned off by you", all_plugins_on: "Plugins turned back on by you",
};
const RUN_OUTCOME: Record<string, string> = { done: "Finished", cancelled: "Cancelled: its permissions changed while it ran", failed: "Failed: nothing was read", refused: "Refused: the plugin is not installed" };
/** How a pack command's output is read, in a sentence for the owner. */
export function describeExtract(command: PackCommand): string {
  const extract = command.extract;
  const read = extract.kind === "line" ? `Reads line ${extract.line ?? 1} of the output as one value`
    : extract.kind === "json" ? `Reads the value at ${extract.path} from JSON output`
    : `Reads the output as a table with the columns ${extract.columns.join(", ")}`;
  return command.healthy ? `${read}. Healthy when it is ${command.healthy.join(" or ")}.` : `${read}.`;
}
/** One log entry as two phrases: what happened and how it ended. */
export function describeActivity(entry: PluginActivity): { action: string; outcome: string } {
  if (entry.action === "open_app") return { action: entry.outcome === "refused" ? "Asked to open an application" : `Opened its application "${entry.detail}"`, outcome: ({ done: "Opened through a private tunnel", cancelled: "Closed: its permissions changed while it opened", failed: "Failed: no tunnel was opened" } as Record<string, string>)[entry.outcome] ?? entry.outcome };
  if (entry.action === "run_command") return { action: entry.outcome === "refused" ? "Asked to run a command" : `Ran its command "${entry.detail}"`, outcome: RUN_OUTCOME[entry.outcome] ?? entry.outcome };
  const owner = OWNER_ACTION[entry.action];
  if (owner) return { action: owner, outcome: entry.action === "enable" ? trustLabel(entry.detail as TrustLevel) : entry.action === "set_permissions" ? (entry.detail || "Everything withdrawn") : entry.action === "revoke_trusted_automation" ? "Now: changes with confirmation" : "" };
  const outcome = entry.outcome === "allowed" ? "Allowed" : entry.outcome === "needs_confirmation" ? "Allowed after your confirmation" : `Refused: ${REFUSAL[entry.detail as Refusal] ?? entry.detail}`;
  return { action: `Asked to use ${entry.action}`, outcome };
}
function ActivityTable({ entries, servers, label }: { entries: PluginActivity[]; servers: MapServer[]; label: string }) {
  if (!entries.length) return <p className="empty-copy">Nothing recorded yet.</p>;
  return <table className="plugin-activity" aria-label={label}><thead><tr><th>When</th><th>Plugin</th><th>Server</th><th>What</th><th>Result</th></tr></thead>
    <tbody>{entries.map(entry => { const text = describeActivity(entry); return <tr key={entry.id} className={entry.outcome}><td><time dateTime={entry.at}>{date(entry.at)}</time></td><td>{entry.plugin_name}</td><td>{entry.server_id ? servers.find(server => server.id === entry.server_id)?.name ?? "Removed server" : "—"}</td><td>{text.action}</td><td>{text.outcome}</td></tr>; })}</tbody></table>;
}

export function statusLabel(status: PluginStatus, trust: TrustLevel | null, allOff = false): string {
  if (allOff) return status === "enabled" ? `Off with all plugins · ${trustLabel(trust)} when on` : "Not enabled";
  if (status === "enabled") return `Enabled · ${trustLabel(trust)}`;
  if (status === "changed_since_review") return "Changed since you reviewed it · switched off";
  return "Not enabled";
}

function PluginPage({ plugin, servers, busy, run, back, allOff, activity }: { plugin: PluginView; servers: MapServer[]; busy: boolean; run: (action: () => Promise<PluginsState>, done: string) => Promise<void>; back: () => void; allOff: boolean; activity: PluginActivity[] }) {
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
    <p className={`plugin-status ${plugin.status}`}><strong>Status:</strong> {statusLabel(plugin.status, plugin.trust, allOff)}</p>

    <h5>What it adds</h5>
    <ul className="plugin-adds">{plugin.contributions.map(entry => <li key={entry.id}><strong>{entry.title}</strong>: {POINT[entry.point]}{entry.command && <span className="pack-shown">Runs exactly this on a server, only when you press Run: <code aria-label={`${entry.title} command`}>{commandLine(entry.command.argv)}</code>
        {entry.command.parameters?.map(parameter => <small key={parameter.name}>{`{${parameter.name}}`} is a {parameter.source} you choose from that server’s inventory.</small>)}<small>{describeExtract(entry.command)} No shell reads the command, and it cannot use sudo.</small>{entry.workload_kind && <small>Shows each row as a workload named by its {entry.workload_kind.name_column} column{entry.workload_kind.state_column ? `, with its ${entry.workload_kind.state_column} as the state` : ""}.</small>}</span>}
        {entry.app && <span className="pack-shown">Opens <code aria-label={`${entry.title} address`}>localhost:{entry.app.port}{entry.app.path ?? "/"}</code> on the server, through the same private tunnel as any application, only when you press Open. It cannot reach another machine.</span>}
        {entry.badge && <span className="pack-shown"><small>Shows the latest result of “{plugin.contributions.find(other => other.id === entry.badge!.observation)?.title ?? entry.badge.observation}” on each server card. It never runs the command itself.</small></span>}
        {entry.relationship && <span className="pack-shown"><small>A {entry.relationship.line ?? "solid"} {entry.relationship.color ?? "green"} line you can choose when documenting a relationship. It applies nothing.</small></span>}</li>)}</ul>

    <h5>What it asks for</h5>
    <p className="muted">This is everything the plugin can ever use. It can use each item only on the servers where you allow it below.</p>
    <ul className="plugin-asks" aria-label="Requested capabilities">{plugin.capabilities.map(entry => <li key={entry.capability}><span className={`kind ${entry.kind}`}>{KIND[entry.kind]}</span><span>{entry.summary}<small>Used by: {entry.used_by.join(", ")} · <code>{entry.capability}</code></small></span></li>)}</ul>
    {asksSudo && <p className="muted">Sudo is never part of a trust level. This plugin asks for it, and it stays refused: InfraWerf cannot yet grant sudo to a plugin.</p>}

    <h5>Trust level</h5>
    {plugin.status === "enabled" ? <>
      <p><strong>{trustLabel(plugin.trust)}.</strong> {TRUST_LEVELS.find(level => level.value === plugin.trust)?.detail}</p>
      <p className="muted">To give more trust, disable the plugin and enable it again.</p>
      <div className="plugin-actions"><button disabled={busy} onClick={() => void run(() => client.disablePlugin(plugin.id), `${plugin.name} disabled.`)}>Disable plugin</button>
        {plugin.trust === "trusted_automation" && <button disabled={busy} onClick={() => void run(() => client.revokeTrustedAutomation(plugin.id), `Trusted automation revoked for ${plugin.name}. Each change now needs your confirmation.`)}>Revoke trusted automation</button>}</div>
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

    <h5>Recent activity</h5>
    <ActivityTable entries={activity} servers={servers} label={`${plugin.name} recent activity`} />
  </section>;
}

/** The Plugins section of the application Settings screen: the list, and one page per plugin. */
export function PluginsSection({ servers, focus, setFocus }: { servers: MapServer[]; focus: string | null; setFocus: (id: string | null) => void }) {
  const [state, setState] = useState<PluginsState | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState("");
  const [message, setMessage] = useState("");
  const [activity, setActivity] = useState<PluginActivity[]>([]);
  const run = useCallback(async (action: () => Promise<PluginsState>, done: string) => {
    setBusy(true); setFailure(""); setMessage("");
    try { setState(await action()); setActivity((await client.pluginActivity(200)).entries); setMessage(done); }
    catch (error) { setFailure(errorMessage(error)); }
    finally { setBusy(false); }
  }, []);
  useEffect(() => { void run(client.plugins, ""); }, [run]);
  const selected = state?.plugins.find(plugin => plugin.id === focus) ?? null;
  return <div className="plugins">
    {failure && <p className="form-error" role="alert">{failure}</p>}{message && <p role="status">{message}</p>}
    {state?.all_off && <p className="form-error" role="alert">All plugins are turned off. Nothing any plugin asks for is allowed until you turn them back on.</p>}
    {!state ? <p className="muted">Looking for plugins…</p> : selected ? <PluginPage plugin={selected} servers={servers} busy={busy} run={run} allOff={state.all_off} activity={activity.filter(entry => entry.plugin_id === selected.id).slice(0, 10)} back={() => { setMessage(""); setFocus(null); }} /> : <>
      <p className="muted">Plugins add to InfraWerf. Finding a plugin allows nothing: you choose what each one may do, on which servers, on its own page.</p>
      <div className="plugin-actions">{state.all_off
        ? <button className="primary" disabled={busy} onClick={() => void run(() => client.setAllPluginsOff(false), "Plugins are back on, with the permissions you had set.")}>Turn plugins back on</button>
        : <button className="danger-button" disabled={busy} onClick={() => void run(() => client.setAllPluginsOff(true), "All plugins are off.")}>Turn all plugins off</button>}
        <span className="muted">One switch for every plugin. Your settings are kept.</span></div>
      {state.plugins.length ? <ul className="plugin-list" aria-label="Installed plugins">{state.plugins.map(plugin => <li key={plugin.id}><div><strong>{plugin.name}</strong> <small>version {plugin.version}</small><span className={`plugin-status ${state.all_off ? "disabled" : plugin.status}`}>{statusLabel(plugin.status, plugin.trust, state.all_off)} · allowed on {plugin.grants.length} {plugin.grants.length === 1 ? "server" : "servers"}</span></div><button aria-label={`Configure ${plugin.name}`} onClick={() => { setMessage(""); setFocus(plugin.id); }}>Configure</button></li>)}</ul>
        : <p className="empty-copy">No plugins found.</p>}
      <p className="muted">A plugin is a folder that contains <code>infrawerf-plugin.json</code>, placed in <code>{state.directory}</code>.</p>
      <button disabled={busy} onClick={() => void run(client.plugins, "Plugin folder read again.")}>Look again</button>
      {state.invalid.length > 0 && <><h5>Folders that are not usable plugins</h5><ul className="plugin-invalid" aria-label="Unusable plugin folders">{state.invalid.map(entry => <li key={entry.folder}><strong>{entry.folder}</strong>{entry.problems.map((problem, index) => <code key={index}>{problem}</code>)}</li>)}</ul></>}
    </>}
  </div>;
}

/** The plugin activity log: every plugin call the core answered and every change you made to a plugin. */
export function PluginActivitySection({ servers }: { servers: MapServer[] }) {
  const [log, setLog] = useState<PluginActivityLog | null>(null);
  const [failure, setFailure] = useState("");
  const load = useCallback(() => { setFailure(""); client.pluginActivity(200).then(setLog, error => setFailure(errorMessage(error))); }, []);
  useEffect(load, [load]);
  return <div className="plugins">
    <p className="muted">Every time a plugin asked to use something, with what InfraWerf answered, and every change you made to a plugin. It records names and outcomes only: never command output, file contents or credentials.{log && ` The newest ${log.kept.toLocaleString()} entries are kept (History retention); the latest ${Math.min(200, log.kept)} are shown.`}</p>
    {failure && <p className="form-error" role="alert">{failure}</p>}
    <button onClick={load}>Refresh activity</button>
    {log && <ActivityTable entries={log.entries} servers={servers} label="Plugin activity log" />}
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
