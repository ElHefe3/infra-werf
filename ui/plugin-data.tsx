import React, { useCallback, useEffect, useState } from "react";
import type { MapServer, PackCommandView, PackObservation, PackOutcome, PackServerState } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { date } from "./workspace-parts";
const client = createWorkspaceClient();

const OUTCOME: Record<PackOutcome, string> = {
  ok: "",
  command_failed: "The command reported a failure, so nothing was read.",
  unreadable: "The command ran, but its output did not hold what this plugin expects.",
  output_limit: "The command printed more than InfraWerf accepts, so nothing was read.",
  timed_out: "The command ran longer than InfraWerf allows, so nothing was read.",
};
const HEALTH = { healthy: "Healthy", unhealthy: "Unhealthy", unknown: "Unknown" } as const;

/** A pack command as the owner reads it. */
export const commandLine = (argv: string[]) => argv.join(" ");

/** What one result says, in words: the health verdict, the value, or why there is none. */
export function describeResult(observation: PackObservation): string {
  const { result } = observation;
  if (result.health) return result.value ? `${HEALTH[result.health]}: ${result.value}` : `${HEALTH[result.health]}. ${OUTCOME[result.outcome]}`.trim();
  if (result.outcome !== "ok") return OUTCOME[result.outcome];
  if (result.value !== null) return result.value;
  return `${result.rows.length} ${result.rows.length === 1 ? "row" : "rows"}${result.truncated ? " (more were not kept)" : ""}`;
}

function Result({ command, observation }: { command: PackCommandView; observation: PackObservation }) {
  const { result } = observation;
  const chosen = Object.values(observation.parameters);
  return <div className="pack-result">
    <p className={`pack-value ${result.health ?? (result.outcome === "ok" ? "ok" : "unknown")}`}>{chosen.length > 0 && <span className="muted">{chosen.join(", ")}: </span>}{describeResult(observation)}</p>
    {result.rows.length > 0 && <div className="table-scroll"><table aria-label={`${command.title} result`}><thead><tr>{result.columns.map(column => <th key={column}>{column.replaceAll("_", " ")}</th>)}</tr></thead>
      <tbody>{result.rows.map((row, index) => <tr key={index}>{row.map((cell, at) => <td key={at}>{cell}</td>)}</tr>)}</tbody></table></div>}
    <small>Read <time dateTime={observation.observed_at}>{date(observation.observed_at)}</time> · {observation.fresh ? "Fresh" : "Stale: run it again for a current answer"} · Source: {command.plugin_name} (<code>{observation.source}</code>) over your SSH access · Kept in memory only</small>
  </div>;
}

/** F21/T043: the commands plugin packs may run on this server, each run only when asked. */
export function PluginData({ server, disabled, onBusy, openPlugin }: { server: MapServer; disabled: boolean; onBusy: (value: boolean) => void; openPlugin: (id: string) => void }) {
  const [state, setState] = useState<PackServerState | null>(null);
  const [running, setRunning] = useState("");
  const [failure, setFailure] = useState("");
  const [choices, setChoices] = useState<Record<string, string>>({});
  const load = useCallback(() => { client.packState(server.id).then(setState, error => setFailure(errorMessage(error))); }, [server.id]);
  useEffect(load, [load]);
  useEffect(() => () => onBusy(false), []);
  const key = (command: PackCommandView) => `${command.plugin_id}.${command.contribution_id}`;
  async function run(command: PackCommandView) {
    const parameters = Object.fromEntries(command.parameters.map(parameter => [parameter.name, choices[`${key(command)}.${parameter.name}`] ?? ""]));
    setRunning(key(command)); setFailure(""); onBusy(true);
    try { setState(await client.runPack(server.id, command.plugin_id, command.contribution_id, parameters)); }
    catch (error) {
      const code = typeof error === "object" && error && "code" in error ? (error as { code: string }).code : "";
      setFailure(code === "denied" ? "InfraWerf refused this command. Either this plugin is no longer allowed on this server, or the chosen workload is not in the inventory this SSH connection collected. Refresh inventory, or check the plugin’s page in Settings." : code === "cancelled" ? "The command was stopped because this plugin’s permissions changed while it ran." : errorMessage(error));
      load();
    } finally { setRunning(""); onBusy(false); }
  }
  const plugins = state ? [...new Map(state.commands.map(command => [command.plugin_id, command.plugin_name])).entries()] : [];
  return <section className="plugin-data" aria-label="Plugin data">
    <div className="section-heading"><div><h3>Plugin data</h3><p>Read-only commands from plugins you enabled and allowed on {server.name}. Each runs only when you press Run, through your own SSH access. Results stay in memory and are never saved.</p></div></div>
    {failure && <p className="form-error" role="alert">{failure}</p>}
    {state && !state.commands.length && <p className="empty-copy">No plugin command is allowed on this server. A plugin’s commands appear here once you enable it and allow both “use your SSH access” and “run a read-only command” on this server, on the plugin’s page in Settings.</p>}
    {plugins.map(([id, name]) => <section className="pack-plugin" key={id} aria-label={`${name} commands`}>
      <header><h4>{name}</h4><button className="text-button" aria-label={`Open ${name} plugin settings`} onClick={() => openPlugin(id)}>Plugin settings</button></header>
      {state?.commands.filter(command => command.plugin_id === id).map(command => {
        const missing = command.parameters.some(parameter => !choices[`${key(command)}.${parameter.name}`]);
        return <article className="pack-command" key={key(command)} aria-label={command.title}>
          <header><strong>{command.title}</strong><span className="kind">{command.health_check ? "Health check" : "Collector"}</span>
            <button className="primary" disabled={disabled || Boolean(running) || missing} aria-label={`Run ${command.title}`} onClick={() => void run(command)}>{running === key(command) ? "Running…" : "Run"}</button></header>
          <code className="pack-argv" aria-label={`${command.title} command`}>{commandLine(command.argv)}</code>
          {command.parameters.map(parameter => { const id = `${key(command)}.${parameter.name}`; return <label key={parameter.name}><span>{`{${parameter.name}}`} is a {parameter.source} from this server’s saved inventory</span>
            <select aria-label={`${command.title} ${parameter.name}`} value={choices[id] ?? ""} onChange={event => setChoices(current => ({ ...current, [id]: event.target.value }))}><option value="">{parameter.choices.length ? "Choose…" : `No ${parameter.source} in saved inventory`}</option>{parameter.choices.map(choice => <option key={choice.id} value={choice.id}>{choice.name}</option>)}</select></label>; })}
          {command.observation ? <Result command={command} observation={command.observation} /> : <p className="muted">Not run yet.</p>}
        </article>;
      })}
    </section>)}
  </section>;
}
