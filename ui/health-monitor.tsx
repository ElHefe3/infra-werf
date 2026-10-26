import React, { useEffect, useState } from "react";
import { createWorkspaceClient } from "./workspace-client";
import type { HealthProbeConfig, HealthSnapshot, MapServer } from "./contracts.gen";

const client = createWorkspaceClient();
const key = (server: MapServer) => `atlas.health.${server.id}`;
const date = (value?: string | null) => value ? new Date(value).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }) : "No observation";
function fresh(report: HealthSnapshot | null) {
  return Boolean(report?.observed_at && Date.now() - Date.parse(report.observed_at) <= 15 * 60 * 1000);
}
function readConfig(server: MapServer): HealthProbeConfig | null {
  try {
    const raw = localStorage.getItem(key(server));
    if (!raw) return null;
    const value = JSON.parse(raw) as HealthProbeConfig;
    if (value.server_id !== server.id || value.host !== server.host) {
      localStorage.removeItem(key(server));
      return null;
    }
    return value;
  } catch {
    localStorage.removeItem(key(server));
    return null;
  }
}

export function HealthMonitor({ server }: { server: MapServer }) {
  const [config, setConfig] = useState<HealthProbeConfig>(() => readConfig(server) ?? {
    server_id: server.id, host: server.host, host_port: 22, service_port: 80,
    service_kind: "http", path: "/health", interval_seconds: 60,
  });
  const [activeConfig, setActiveConfig] = useState<HealthProbeConfig | null>(() => readConfig(server));
  const [report, setReport] = useState<HealthSnapshot | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [monitoring, setMonitoring] = useState(() => Boolean(readConfig(server)));

  useEffect(() => {
    const saved = readConfig(server);
    if (!saved) { setReport(null); setActiveConfig(null); setMonitoring(false); return; }
    setConfig(saved);
    setActiveConfig(saved);
    let alive = true;
    setMonitoring(true);
    void client.configureHealth(server.id, saved).then(value => { if (alive) setReport(value); }).catch(() => {});
    return () => { alive = false; };
  }, [server.id, server.host]);
  useEffect(() => {
    if (!monitoring) return;
    let alive = true;
    const poll = () => void client.healthStatus(server.id).then(value => { if (alive) setReport(value); }).catch(() => {});
    poll();
    const timer = window.setInterval(poll, 2000);
    return () => { alive = false; window.clearInterval(timer); };
  }, [monitoring, server.id]);

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault(); setBusy(true); setError("");
    const data = new FormData(event.currentTarget);
    const next: HealthProbeConfig = {
      server_id: server.id,
      host: server.host,
      host_port: Number(data.get("hostPort")),
      service_port: Number(data.get("servicePort")),
      service_kind: String(data.get("kind")) as HealthProbeConfig["service_kind"],
      path: String(data.get("kind")) === "http" ? String(data.get("path") ?? "") : null,
      interval_seconds: Number(data.get("interval")),
    };
    try {
      const value = await client.configureHealth(server.id, next);
      localStorage.setItem(key(server), JSON.stringify(next));
      setConfig(next); setActiveConfig(next); setReport(value); setMonitoring(true);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "The health check could not be configured.");
    } finally { setBusy(false); }
  }

  async function stop() {
    setBusy(true); setError("");
    try {
      await client.cancelHealth(server.id);
      localStorage.removeItem(key(server)); setReport(null); setActiveConfig(null); setMonitoring(false);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "The health check could not be stopped.");
    } finally { setBusy(false); }
  }

  const checked = fresh(report) ? date(report?.observed_at) : report?.observed_at ? `Stale · ${date(report.observed_at)}` : "No observation yet";
  const resultConfig = activeConfig ?? config;
  return <div className="health-panel">
    <div className="section-heading"><div><h3>Network health</h3><p>Host reachability and service response are checked separately. Results stay in memory.</p></div></div>
    <form onSubmit={save} className="health-form">
      <label>Host TCP port<input name="hostPort" type="number" min="1" max="65535" required value={config.host_port} onChange={event => setConfig({ ...config, host_port: Number(event.target.value) })} /></label>
      <label>Service check<select name="kind" value={config.service_kind} onChange={event => setConfig({ ...config, service_kind: event.target.value as HealthProbeConfig["service_kind"] })}><option value="http">HTTP GET</option><option value="tcp">TCP connect</option></select></label>
      <label>Service port<input name="servicePort" type="number" min="1" max="65535" required value={config.service_port} onChange={event => setConfig({ ...config, service_port: Number(event.target.value) })} /></label>
      {config.service_kind === "http" && <label>HTTP path<input name="path" maxLength={512} required value={config.path ?? "/health"} onChange={event => setConfig({ ...config, path: event.target.value })} /></label>}
      <label>Check interval (seconds)<input name="interval" type="number" min="15" max="3600" required value={config.interval_seconds} onChange={event => setConfig({ ...config, interval_seconds: Number(event.target.value) })} /></label>
      <div className="form-actions"><button className="primary" disabled={busy}>{busy ? "Saving…" : "Save & start checks"}</button><button type="button" disabled={busy || !report} onClick={() => void stop()}>Stop checks</button></div>
    </form>
    {error && <p role="alert" className="form-error">{error}</p>}
    <dl className="system-grid health-results"><div><dt>Host · TCP {resultConfig.host_port}</dt><dd><span className={`state-pill ${report?.host_reachability.state ?? "unknown"}`}>{report?.host_reachability.state ?? "unknown"}</span></dd><dd>Last checked · {date(report?.host_reachability.last_checked_at)}</dd><dd>Last success · {date(report?.host_reachability.last_success_at)}</dd><dd>Last failure · {date(report?.host_reachability.last_failure_at)}</dd></div><div><dt>Service · {resultConfig.service_kind.toUpperCase()} {resultConfig.service_port}</dt><dd><span className={`state-pill ${report?.service.state ?? "unknown"}`}>{report?.service.state ?? "unknown"}</span></dd><dd>Last checked · {date(report?.service.last_checked_at)}</dd><dd>Last success · {date(report?.service.last_success_at)}</dd><dd>Last failure · {date(report?.service.last_failure_at)}</dd></div></dl>
    <p className="muted">Observation · {checked}. HTTP checks send an unauthenticated plain HTTP GET without following redirects. Each service target is the server address shown above.</p>
  </div>;
}
