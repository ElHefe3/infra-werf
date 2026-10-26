import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { MapServer, WorkloadAction, WorkloadControlPreview, WorkloadControlReceipt, WorkloadInspection, WorkloadIdentity } from "./contracts.gen";
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
const actionLabel = (action: WorkloadAction) => ({ start: "Start", stop: "Stop", restart: "Restart" }[action]);
const observedLabel = (state: WorkloadControlReceipt["observed_state"]) => ({
  running: "Running",
  stopped: "Stopped",
  permission_denied: "Permission denied",
  unavailable: "Unavailable",
}[state]);
const uncertainControlMessage = "The action may have been sent, but its result is unknown. Reconnect and inspect the workload before trying again.";

export function WorkloadInspectionPanel({ server, onBusy }: { server: MapServer; onBusy: (busy: boolean) => void }) {
  const access = useProtectedSession(server, onBusy);
  const [selectedId, setSelectedId] = useState("");
  const [inspection, setInspection] = useState<WorkloadInspection | null>(null);
  const [inspectionFailure, setInspectionFailure] = useState("");
  const [inspecting, setInspecting] = useState("");
  const [controlWorkload, setControlWorkload] = useState<WorkloadIdentity | null>(null);
  const [controlAction, setControlAction] = useState<WorkloadAction>("start");
  const [controlPreview, setControlPreview] = useState<WorkloadControlPreview | null>(null);
  const [controlReceipt, setControlReceipt] = useState<WorkloadControlReceipt | null>(null);
  const [controlFailure, setControlFailure] = useState("");
  const [controlNotice, setControlNotice] = useState("");
  const [uncertainControlNotice, setUncertainControlNotice] = useState("");
  const [controlBusy, setControlBusy] = useState(false);
  const controlWorking = useRef(false);
  const controlOperation = useRef<string | null>(null);
  const password = useRef<HTMLInputElement>(null);

  useLayoutEffect(() => {
    setInspection(null);
    setInspectionFailure("");
    setInspecting("");
    setSelectedId("");
    setControlWorkload(null);
    setControlPreview(null);
    setControlReceipt(null);
    setControlFailure("");
    setControlNotice("");
    setControlBusy(false);
    controlWorking.current = false;
    controlOperation.current = null;
    if (password.current) password.current.value = "";
  }, [access.session?.session_id]);

  useEffect(() => {
    if (!controlPreview) return;
    const preview = controlPreview;
    const timer = setTimeout(() => {
      if (controlWorking.current) return;
      const session = access.active.current;
      if (session) void client.cancelElevation(server.id, session.session_id, preview.operation_id).catch(() => {});
      controlOperation.current = null;
      setControlPreview(null);
      setControlNotice("This review expired. Review the workload action again before confirming.");
    }, Math.max(0, Date.parse(preview.expires_at) - Date.now()));
    return () => clearTimeout(timer);
  }, [controlPreview?.operation_id]);

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
    if (controlPreview && controlOperation.current) setUncertainControlNotice(uncertainControlMessage);
    access.lock();
    setInspection(null);
    setInspectionFailure("");
    setSelectedId("");
    clearControl();
  }

  function clearControl() {
    if (password.current) password.current.value = "";
    setControlPreview(null);
    setControlReceipt(null);
    setControlFailure("");
    setControlNotice("");
    setControlWorkload(null);
    controlOperation.current = null;
    controlWorking.current = false;
    setControlBusy(false);
  }

  function reviewControl(workload: WorkloadIdentity) {
    const session = access.active.current;
    if (!session || controlWorking.current) return;
    controlWorking.current = true;
    setControlBusy(true);
    setControlFailure("");
    setControlNotice("");
    setControlReceipt(null);
    setControlWorkload(workload);
    const epoch = access.generation.current;
    void client.previewWorkloadControl(server.id, session.session_id, workload.id, controlAction).then(async preview => {
      if (epoch !== access.generation.current || access.active.current?.session_id !== session.session_id) {
        await client.cancelElevation(server.id, session.session_id, preview.operation_id).catch(() => {});
        return;
      }
      controlOperation.current = preview.operation_id;
      setControlPreview(preview);
      access.arm(session);
    }).catch(failure => {
      if (epoch === access.generation.current) setControlFailure(errorMessage(failure));
    }).finally(() => {
      if (epoch === access.generation.current) {
        controlWorking.current = false;
        setControlBusy(false);
      }
    });
  }

  async function confirmControl(event: React.FormEvent) {
    event.preventDefault();
    const session = access.active.current;
    if (!session || !controlPreview || controlWorking.current) return;
    const secret = password.current?.value ?? "";
    if (controlPreview.requires_sudo && !secret) return;
    controlWorking.current = true;
    setControlBusy(true);
    onBusy(true);
    setControlFailure("");
    setControlNotice("");
    const epoch = access.generation.current;
    const currentPreview = controlPreview;
    controlOperation.current = currentPreview.operation_id;
    const request = client.confirmWorkloadControl(server.id, session.session_id, currentPreview.operation_id, controlPreview.requires_sudo ? secret : undefined);
    if (password.current) password.current.value = "";
    try {
      const receipt = await request;
      if (epoch === access.generation.current && access.active.current?.session_id === session.session_id) {
        setControlReceipt(receipt);
        controlOperation.current = null;
        setControlPreview(null);
        access.arm(session);
      } else {
        setUncertainControlNotice(uncertainControlMessage);
      }
    } catch (failure) {
      if (epoch !== access.generation.current) {
        setUncertainControlNotice(uncertainControlMessage);
        return;
      }
      const code = (failure as { code?: string })?.code;
      if (code === "outcome_unknown") {
        access.lock("Session locked because the workload action result is unknown. Reconnect and inspect the workload before trying again.");
      } else if (["expired_session", "io_failure", "host_trust_failure", "timed_out", "output_limit"].includes(code ?? "")) access.lock();
      if (["cancelled", "timed_out", "outcome_unknown", "io_failure"].includes(code ?? "")) {
        setUncertainControlNotice(uncertainControlMessage);
      } else if (code === "unauthenticated") {
        setControlFailure("The fresh sudo check failed. No action was authorized; review again and enter the password for this action.");
      } else if (code === "denied") {
        setControlFailure("The server did not authorize this control action. Atlas did not dispatch the workload command.");
      } else {
        setControlFailure(errorMessage(failure));
      }
      controlOperation.current = null;
      setControlPreview(null);
    } finally {
      if (epoch === access.generation.current) {
        controlWorking.current = false;
        setControlBusy(false);
        onBusy(false);
      }
    }
  }

  async function cancelControl() {
    if (password.current) password.current.value = "";
    const session = access.active.current;
    const operation = controlOperation.current;
    if (session && operation) {
      try { await client.cancelElevation(server.id, session.session_id, operation); }
      catch (failure) { setControlFailure(errorMessage(failure)); return; }
    }
    if (controlWorking.current) {
      setControlNotice("Cancellation requested. If the action had already been sent, inspect the workload to verify its state.");
    } else {
      clearControl();
    }
  }

  const workloads = access.session?.workloads ?? [];
  const inventoryBoundToHost = server.inventory?.source_host === server.host;
  return <div className="workload-inspection">
    <div className="section-heading"><div><h3>Live workload details & logs</h3><p>On-demand status and the latest 50 log lines, capped at 8 KiB. Results stay in this protected view.</p></div>
      {access.session ? <button onClick={lock}>Lock protected view</button> : <button className="primary" disabled={access.busy || !inventoryBoundToHost} onClick={() => void access.connect()}>{access.busy ? "Opening…" : "Open protected view"}</button>}
    </div>
    {access.error && <p className="form-error" role="alert">{access.error}</p>}
    {access.notice && <p className="inventory-refresh-status muted" role="status">{access.notice}</p>}
    {uncertainControlNotice && <p className="form-error workload-control-uncertain" role="alert">{uncertainControlNotice} <button type="button" onClick={() => setUncertainControlNotice("")}>Dismiss</button></p>}
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
          <td>{workload.supported ? <><button disabled={Boolean(inspecting) || controlBusy} onClick={() => void inspect(workload)}>{inspecting === workload.id ? "Inspecting…" : selectedId === workload.id && inspection ? "Refresh details" : "Inspect"}</button><button disabled={controlBusy} onClick={() => { setControlReceipt(null); setControlWorkload(workload); setControlPreview(null); setControlAction("start"); setControlFailure(""); setControlNotice(""); }}>Control…</button></> : <span className="muted">Unsupported scope</span>}</td>
        </tr>)}
      </tbody></table>{!workloads.length && <p className="empty-copy">No saved systemd, Podman, or Docker workloads are available for this host.</p>}</div>
      {inspectionFailure && <p className="form-error" role="alert">Inspection failed: {inspectionFailure}</p>}
      {inspection && <section className="workload-result" aria-label="Protected workload result">
        <div className="workload-result-heading"><div><h4>{workloads.find(item => item.id === inspection.workload_id)?.name ?? "Workload"}</h4><span className={`state-pill workload-${inspection.state}`}>{stateLabel(inspection.state)}</span></div><small>Checked {new Date(inspection.inspected_at).toLocaleString()}</small></div>
        {inspection.details.length > 0 && <dl className="workload-detail-grid">{inspection.details.map(item => <div key={item.label}><dt>{item.label}</dt><dd>{item.value || "—"}</dd></div>)}</dl>}
        <div className="workload-logs-heading"><h4>{logLabel(inspection.logs_state)}</h4>{inspection.logs_truncated && <span className="muted">Older output trimmed to the 8 KiB limit</span>}</div>
        {inspection.logs_state === "available" ? <pre className="workload-logs" aria-label="Protected workload logs">{inspection.logs || "No log lines were returned."}</pre> : <p className="muted">{inspection.logs_state === "permission_denied" ? "The SSH account cannot read these logs." : "The runtime could not return logs for this workload."}</p>}
      </section>}
      {controlWorkload && <section className="workload-result workload-control" aria-label="Reviewed workload control">
        <div className="workload-result-heading"><div><h4>Control {controlWorkload.name}</h4><span className="muted">{controlWorkload.kind} · {controlWorkload.scope} · {controlWorkload.runtime}</span></div><button disabled={controlBusy} onClick={() => void cancelControl()}>Close</button></div>
        {!controlPreview && !controlReceipt && <div className="workload-control-review">
          <label>Action<select aria-label="Workload action" value={controlAction} disabled={controlBusy} onChange={event => setControlAction(event.target.value as WorkloadAction)}><option value="start">Start</option><option value="stop">Stop</option><option value="restart">Restart</option></select></label>
          <p>Review the exact host, runtime scope, workload and action before Atlas executes it.</p>
          <button className="primary" disabled={controlBusy} onClick={() => reviewControl(controlWorkload)}>{controlBusy ? "Preparing review…" : "Review action"}</button>
        </div>}
        {controlPreview && <form className="file-change-review" aria-label="Confirm workload action" onSubmit={event => void confirmControl(event)}>
          <dl><dt>Server</dt><dd>{server.name} · {controlPreview.server}</dd><dt>SSH account</dt><dd>{controlPreview.endpoint}</dd><dt>Workload</dt><dd>{controlPreview.workload.name} · {controlPreview.workload.kind}</dd><dt>Scope / runtime</dt><dd>{controlPreview.workload.scope} · {controlPreview.workload.runtime}</dd><dt>Action</dt><dd>{actionLabel(controlPreview.action)}</dd><dt>Elevation</dt><dd>{controlPreview.requires_sudo ? "Fresh sudo password required for this action" : "Runs as the authenticated SSH account"}</dd></dl>
          {controlPreview.requires_sudo && <label>Password for this action<input ref={password} type="password" aria-label="Password for this action" autoComplete="off" maxLength={1024} required disabled={controlBusy} /></label>}
          <p>Atlas will run only this reviewed action, then observe the workload again. If the result is unknown, inspect the workload before retrying.</p>
          <button className="primary" type="submit" disabled={controlBusy}>{controlBusy ? "Executing and observing…" : controlPreview.requires_sudo ? "Confirm with fresh sudo" : "Confirm action"}</button>
          <button type="button" onClick={() => void cancelControl()}>{controlBusy ? "Cancel action" : "Cancel review"}</button>
        </form>}
        {controlFailure && <p className="form-error" role="alert">{controlFailure}</p>}
        {controlNotice && <p role="status">{controlNotice}</p>}
        {controlReceipt && <div role="status" className={`workload-action-result workload-action-${controlReceipt.outcome}`}><span className="state-pill">{controlReceipt.outcome === "completed" ? "Observed complete" : controlReceipt.outcome === "failed" ? "Action failed" : "Outcome unknown"}</span><span>{controlReceipt.message}</span><span>Observed state: {observedLabel(controlReceipt.observed_state)} · {new Date(controlReceipt.observed_at).toLocaleString()}</span></div>}
      </section>}
    </>}
  </div>;
}
