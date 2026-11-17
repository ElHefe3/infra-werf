import React, { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { JobAsk, JobEntry, JobRunView, JobsState, MapServer } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { date } from "./workspace-parts";
import { Dialog } from "./dialog";
import { hideViews } from "./plugin-view";
import { describeJobRun, MIN_SCHEDULE_MINUTES, scheduleLabel } from "./plugin-jobs-model";
const client = createWorkspaceClient();

// Plugin jobs (S12/F24/T049, owner decision D061). A job is the plugin's own
// sandboxed logic; its changing actions are InfraWerf's workload controls.
// The core decides every call; these screens start runs, show them, set the
// owner's schedules and exceptions, and ask the owner before an action.

const CONTROL = { start: "Start", stop: "Stop", restart: "Restart" } as const;
const TARGET = { service: "a service", container: "a container" } as const;

/** What a job may do, as the plugin's page shows it before enabling. */
export function JobConsent({ job }: { job: NonNullable<JobEntry["job"]> }) {
  return <span className="pack-shown"><small>Runs its own logic, <code>{job.module}</code>, in InfraWerf’s sandbox. It reads only what you allow it on a server.</small>
    {(job.actions ?? []).length > 0 && <ul className="job-actions" aria-label="Actions it may ask for">{(job.actions ?? []).map(action => <li key={action.id}><strong>{action.title}</strong>: {CONTROL[action.control]} {TARGET[action.target]} from that server’s inventory. {action.effect}</li>)}</ul>}
    {(job.actions ?? []).length > 0 && <small>An action runs at once only if you give the plugin trusted automation; otherwise InfraWerf asks you first, and a scheduled run cannot make it. An action that needs sudo needs your exception for it on that server, and your password every time.</small>}</span>;
}

/** Schedule and sudo-exception settings for one plugin's jobs on one server, on the plugin's page. */
export function JobSettings({ pluginId, server, busy }: { pluginId: string; server: MapServer; busy: boolean }) {
  const [state, setState] = useState<JobsState | null>(null);
  const [minutes, setMinutes] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState(""), [message, setMessage] = useState("");
  useEffect(() => { client.pluginJobs(server.id).then(setState, () => {}); }, [server.id, busy]);
  const jobs = state?.jobs.filter(job => job.plugin_id === pluginId) ?? [];
  if (!jobs.length) return null;
  async function act(work: () => Promise<JobsState>, done: string) {
    setFailure(""); setMessage("");
    try { setState(await work()); setMessage(done); } catch (error) { setFailure(errorMessage(error)); }
  }
  return <div className="job-settings" aria-label={`Jobs on ${server.name}`}>
    {jobs.map(job => { const key = `${job.contribution_id}`; const every = minutes[key] ?? String(job.schedule?.every_minutes ?? 15);
      return <div key={key} className="job-setting">
        <strong>{job.title}</strong>
        <p className="muted">{scheduleLabel(job.schedule)}</p>
        <label>Every <input aria-label={`${job.title} interval on ${server.name}`} type="number" min={MIN_SCHEDULE_MINUTES} max={1440} value={every} onChange={e => setMinutes(current => ({ ...current, [key]: e.target.value }))} /> minutes</label>
        <div className="plugin-actions">
          <button disabled={busy} onClick={() => void act(() => client.setJobSchedule(server.id, pluginId, job.contribution_id, Number(every)), `${job.title} runs every ${every} minutes on ${server.name} while InfraWerf is open.`)}>{job.schedule ? "Change schedule" : "Turn on schedule"}</button>
          {job.schedule && <button disabled={busy} onClick={() => void act(() => client.setJobSchedule(server.id, pluginId, job.contribution_id, null), `${job.title}'s schedule on ${server.name} is off.`)}>Turn off schedule</button>}
        </div>
        {(job.job.actions ?? []).map(action => { const granted = job.elevation_exceptions.includes(action.id);
          return <label key={action.id} className="job-exception"><input type="checkbox" disabled={busy} checked={granted} onChange={event => void act(() => client.setJobElevation(server.id, pluginId, action.id, event.target.checked), event.target.checked ? `Sudo exception for “${action.title}” on ${server.name}: granted. Your password is still asked every time.` : `Sudo exception for “${action.title}” on ${server.name}: withdrawn.`)} />
            Allow sudo for “{action.title}” on {server.name} (asks your password every time)</label>; })}
      </div>; })}
    {failure && <p className="form-error" role="alert">{failure}</p>}{message && <p role="status">{message}</p>}
  </div>;
}

/** The jobs allowed on a server, in its Plugin data tab. */
export function PluginJobs({ server, disabled, openPlugin }: { server: MapServer; disabled: boolean; openPlugin: (id: string) => void }) {
  const [state, setState] = useState<JobsState | null>(null);
  const [failure, setFailure] = useState("");
  const load = useCallback(() => { client.pluginJobs(server.id).then(setState, error => setFailure(errorMessage(error))); }, [server.id]);
  useEffect(() => { load(); const timer = window.setInterval(load, 1500); return () => window.clearInterval(timer); }, [load]);
  if (!state || !state.jobs.length) return null;
  const latest = (job: JobEntry): JobRunView | undefined => state.runs.find(run => run.plugin_id === job.plugin_id && run.contribution_id === job.contribution_id);
  async function start(job: JobEntry) {
    setFailure("");
    try { await client.runJob(server.id, job.plugin_id, job.contribution_id); load(); }
    catch (error) { setFailure(errorMessage(error)); }
  }
  return <section className="plugin-jobs" aria-label="Plugin jobs">
    <h4>Jobs</h4>
    <p className="muted">A plugin’s own logic, run in InfraWerf’s sandbox with only what you allowed on {server.name}. Changing actions ask you first unless the plugin has trusted automation.</p>
    {failure && <p className="form-error" role="alert">{failure}</p>}
    {state.jobs.map(job => { const run = latest(job); const running = run?.state === "running";
      return <article key={`${job.plugin_id}.${job.contribution_id}`} className="pack-command" aria-label={`${job.title} job`}>
        <header><strong>{job.title}</strong><span className="kind">Job · {job.plugin_name}</span>
          {running ? <button onClick={() => run && void client.cancelJob(run.run_id).then(load)}>Stop</button>
            : <button className="primary" disabled={disabled} aria-label={`Run ${job.title}`} onClick={() => void start(job)}>Run now</button>}</header>
        <small className="muted">{scheduleLabel(job.schedule)} · <button className="text-button" onClick={() => openPlugin(job.plugin_id)}>Schedule and sudo settings</button></small>
        {run ? <div className="job-run" aria-label={`${job.title} last run`}>
          <p className={`pack-value ${run.state}`}>{describeJobRun(run)}</p>
          {run.actions.length > 0 && <ul aria-label={`${job.title} actions`}>{run.actions.map((action, index) => <li key={index}>{(job.job.actions ?? []).find(a => a.id === action.action_id)?.title ?? action.action_id} on {action.workload || "—"}: {action.outcome}{action.message ? ` · ${action.message}` : ""}</li>)}</ul>}
          {run.outcome && run.outcome.logs.length > 0 && <pre className="job-logs" aria-label={`${job.title} log`}>{run.outcome.logs.join("\n")}</pre>}
          <small>Started <time dateTime={run.started_at}>{date(run.started_at)}</time>{run.attended ? " by you" : " by its schedule"}{run.finished_at ? ` · ended ${date(run.finished_at)}` : ""}</small>
        </div> : <p className="muted">Not run yet.</p>}
      </article>; })}
  </section>;
}

/** InfraWerf's own dialog for an action a job asks for. */
export function JobAskDialog() {
  const [queue, setQueue] = useState<JobAsk[]>([]);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false), [ready, setReady] = useState(false);
  const decline = useRef<HTMLButtonElement>(null);
  const ask = queue[0] ?? null;
  useEffect(() => {
    let disposed = false, unlisten: (() => void) | undefined;
    const add = (incoming: JobAsk) => { hideViews(true); setQueue(current => current.some(a => a.id === incoming.id) ? current : [...current, incoming]); };
    void listen<JobAsk>("atlas-plugin-job-ask", event => add(event.payload)).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {});
    void client.jobAsks().then(asks => asks.forEach(add)).catch(() => {});
    return () => { disposed = true; unlisten?.(); };
  }, []);
  useEffect(() => {
    if (!ask) return;
    setReady(false); setPassword("");
    requestAnimationFrame(() => decline.current?.focus());
    const timer = window.setTimeout(() => setReady(true), 1000);
    return () => window.clearTimeout(timer);
  }, [ask?.id]);
  if (!ask) return null;
  async function answer(run: boolean) {
    if (!ask) return;
    setBusy(true);
    try { await client.answerJob(ask.id, run, run && ask.requires_sudo ? password : null); } catch { /* the run sees the outcome */ }
    finally { setBusy(false); setPassword(""); setQueue(current => { const rest = current.slice(1); if (!rest.length) hideViews(false); return rest; }); }
  }
  return <Dialog title="A plugin job asks to make a change" close={() => void answer(false)} busy={busy}>
    <p><strong>{ask.plugin_name}</strong>’s job <strong>{ask.job_title}</strong> asks to <strong>{CONTROL[ask.control].toLowerCase()}</strong> {TARGET[ask.target]} <strong>{ask.workload}</strong> on <strong>{ask.server_name}</strong>.</p>
    <p className="muted">{ask.action_title}: {ask.effect}</p>
    {ask.requires_sudo && <label>Sudo password for this action<input aria-label="Sudo password for this action" type="password" autoComplete="off" value={password} onChange={e => setPassword(e.target.value)} /><small>Used for this action only, then forgotten. It is never stored or shown to the plugin.</small></label>}
    <p className="muted">It runs through the same reviewed control and checks as the Services tab. Nothing happens unless you press Run.</p>
    <div className="form-actions"><button ref={decline} disabled={busy} onClick={() => void answer(false)}>Decline</button>
      <button className="primary" disabled={busy || !ready || (ask.requires_sudo && !password)} onClick={() => void answer(true)}>{busy ? "Running…" : ready ? "Run" : "Run (wait…)"}</button></div>
  </Dialog>;
}
