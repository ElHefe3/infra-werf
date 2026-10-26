import { useLayoutEffect, useState } from "react";
import type { MapServer, WorkloadInspection, WorkloadIdentity } from "./contracts.gen";
import { errorMessage } from "./map-model";
import { createWorkspaceClient } from "./workspace-client";
import { useProtectedSession } from "./protected-session";

const client = createWorkspaceClient();
const stateLabel = (state: WorkloadInspection["state"]) => ({
  running: "Running",
  stopped: "Stopped",
  permission_denied: "Permission denied",
  unavailable: "Unavailable",
}[state]);
const logLabel = (state: WorkloadInspection["logs_state"]) => ({
  available: "Recent logs",
  permission_denied: "Log access denied",
  unavailable: "Logs unavailable",
}[state]);

export function WorkloadInspectionPanel({ server, onBusy }: { server: MapServer; onBusy: (busy: boolean) => void }) {
  const access = useProtectedSession(server, onBusy);
  const [selectedId, setSelectedId] = useState("");
  const [inspection, setInspection] = useState<WorkloadInspection | null>(null);
  const [inspectionFailure, setInspectionFailure] = useState("");
  const [inspecting, setInspecting] = useState("");

  useLayoutEffect(() => {
    setInspection(null);
    setInspectionFailure("");
    setInspecting("");
    setSelectedId("");
  }, [access.session?.session_id]);

  async function inspect(workload: WorkloadIdentity) {
    const session = access.session;
    if (!session || inspecting) return;
    const epoch = access.generation.current;
    setSelectedId(workload.id);
    setInspection(null);
    setInspectionFailure("");
    setInspecting(workload.id);
    onBusy(true);
    try {
      const value = await client.inspectWorkload(server.id, session.session_id, workload.id);
      if (epoch === access.generation.current && access.active.current?.session_id === session.session_id) setInspection(value);
    } catch (failure) {
      if (epoch === access.generation.current && access.active.current?.session_id === session.session_id) setInspectionFailure(errorMessage(failure));
    } finally {
      if (epoch === access.generation.current) {
        setInspecting("");
        onBusy(false);
      }
    }
  }

  function lock() {
    access.lock();
    setInspection(null);
    setInspectionFailure("");
    setSelectedId("");
  }

  const workloads = access.session?.workloads ?? [];
  const inventoryBoundToHost = server.inventory?.source_host === server.host;
  return <div className="workload-inspection">
    <div className="section-heading"><div><h3>Live workload details & logs</h3><p>On-demand status and the latest 50 log lines, capped at 8 KiB. Results stay in this protected view.</p></div>
      {access.session ? <button onClick={lock}>Lock protected view</button> : <button className="primary" disabled={access.busy || !inventoryBoundToHost} onClick={() => void access.connect()}>{access.busy ? "Opening…" : "Open protected view"}</button>}
    </div>
    {access.error && <p className="form-error" role="alert">{access.error}</p>}
    {access.notice && <p className="inventory-refresh-status muted" role="status">{access.notice}</p>}
    {!server.inventory && <p className="empty-copy">Refresh approved inventory before inspecting workloads.</p>}
    {server.inventory && !inventoryBoundToHost && <p className="inventory-refresh-status muted" role="status">Live inspection is unavailable because this observation is imported or bound to another SSH host entry. Complete an authenticated inventory refresh for {server.host} first.</p>}
    {access.session && <>
      <p className="muted workload-session-note">Authenticated as {access.session.profile.user} on {server.host}. Locking, expiry, or leaving this view clears details and logs.</p>
      {server.inventory && !access.session.workload_inventory_matches_session && <p className="inventory-refresh-status muted" role="status">The saved workload list was collected with a different SSH connection. Refresh approved inventory to inspect workloads with this connection.</p>}
      <div className="table-scroll"><table><thead><tr><th>Workload</th><th>Scope / runtime</th><th>Saved state</th><th>Live result</th></tr></thead><tbody>
        {workloads.map(workload => <tr key={workload.id}>
          <td><strong>{workload.name}</strong><small>{workload.kind}</small></td>
          <td>{workload.scope}<small>{workload.runtime}</small></td>
          <td><span className="state-pill">{workload.reported_state || "Unknown"}</span>{workload.reported_detail && <small>{workload.reported_detail}</small>}</td>
          <td>{workload.supported ? <button disabled={Boolean(inspecting)} onClick={() => void inspect(workload)}>{inspecting === workload.id ? "Inspecting…" : selectedId === workload.id && inspection ? "Refresh details" : "Inspect"}</button> : <span className="muted">Unsupported scope</span>}</td>
        </tr>)}
      </tbody></table>{!workloads.length && <p className="empty-copy">No saved systemd, Podman, or Docker workloads are available for this host.</p>}</div>
      {inspectionFailure && <p className="form-error" role="alert">Inspection failed: {inspectionFailure}</p>}
      {inspection && <section className="workload-result" aria-label="Protected workload result">
        <div className="workload-result-heading"><div><h4>{workloads.find(item => item.id === inspection.workload_id)?.name ?? "Workload"}</h4><span className={`state-pill workload-${inspection.state}`}>{stateLabel(inspection.state)}</span></div><small>Checked {new Date(inspection.inspected_at).toLocaleString()}</small></div>
        {inspection.details.length > 0 && <dl className="workload-detail-grid">{inspection.details.map(item => <div key={item.label}><dt>{item.label}</dt><dd>{item.value || "—"}</dd></div>)}</dl>}
        <div className="workload-logs-heading"><h4>{logLabel(inspection.logs_state)}</h4>{inspection.logs_truncated && <span className="muted">Older output trimmed to the 8 KiB limit</span>}</div>
        {inspection.logs_state === "available" ? <pre className="workload-logs" aria-label="Protected workload logs">{inspection.logs || "No log lines were returned."}</pre> : <p className="muted">{inspection.logs_state === "permission_denied" ? "The SSH account cannot read these logs." : "The runtime could not return logs for this workload."}</p>}
      </section>}
    </>}
  </div>;
}
