import React, { useState } from "react";
import type { MonitoringState } from "./contracts.gen";
import type { WorkspaceClient } from "./workspace-client";
import type { Bound } from "./extensions-model";
import { errorMessage } from "./map-model";

// Application-wide Settings (D055, T052): everything about InfraWerf itself
// that is not tied to one server. The core registers each section at the
// app_settings_section extension point; this file draws them. Settings about
// one server stay in that server's own Settings tab.

export type AppSettingsContext = {
  monitoring: MonitoringState | null;
  client: WorkspaceClient;
  /** A change is being saved; controls should wait. */
  busy: boolean;
  /** Runs a change, then reloads and reports the outcome in the screen's status line. */
  act: (action: () => Promise<unknown>, message: string) => Promise<void>;
};
export type AppSettingsRenderer = (context: AppSettingsContext) => React.ReactNode;

export const HISTORY_CHOICES = [200, 1000, 5000, 10000] as const;

export const appSettings: Record<string, AppSettingsRenderer> = {
  "builtin.app-settings.history-retention": ({ monitoring, client, busy, act }) => <>
    <p className="muted">How many recent reachability, inventory and lifecycle events InfraWerf keeps on this PC. Older events are removed.</p>
    <label className="history-retention">Keep recent history<select aria-label="History retention" disabled={busy || !monitoring} value={monitoring?.history_limit ?? 1000} onChange={e => void act(() => client.historyLimit(Number(e.target.value)), "History retention saved.")}>{HISTORY_CHOICES.map(n => <option key={n} value={n}>{n.toLocaleString()} events</option>)}</select></label>
  </>,
  "builtin.app-settings.ssh-discovery": ({ client, busy, act }) => <>
    <p className="muted">Discovery reads literal aliases from your primary SSH configuration every minute. Includes and wildcard aliases are excluded. Candidates are not contacted until you activate monitoring. Addresses are never used to merge server identities.</p>
    <button disabled={busy} onClick={() => void act(client.discoverServers, "SSH configuration checked for new candidates.")}>Discover configured servers</button>
  </>,
};

export function SettingsScreen({ sections, monitoring, client, reload }: { sections: Bound<AppSettingsRenderer>[]; monitoring: MonitoringState | null; client: WorkspaceClient; reload: () => Promise<void> }) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState("");
  const [message, setMessage] = useState("");
  async function act(action: () => Promise<unknown>, done: string) {
    setBusy(true); setFailure(""); setMessage("");
    try { await action(); await reload(); setMessage(done); }
    catch (e) { setFailure(errorMessage(e)); }
    finally { setBusy(false); }
  }
  const context: AppSettingsContext = { monitoring, client, busy, act };
  return <div className="settings-screen"><div className="section-heading"><div><h2>Settings</h2><p>Settings for InfraWerf itself. Settings for one server are in that server’s workspace.</p></div></div>
    {failure && <p className="form-error" role="alert">{failure}</p>}{message && <p role="status">{message}</p>}
    {sections.map(({ contribution, renderer }) => <section className="settings-section" key={contribution.id} aria-label={contribution.title}><h3>{contribution.title}</h3>{renderer(context)}</section>)}
    {!sections.length && <p className="empty-copy">No application settings are available.</p>}
  </div>;
}
