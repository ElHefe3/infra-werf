import React, { useState } from "react";
import { Secret } from "./pin";
import type { MapServer, MapState, MonitoringPolicy, MonitoringRecord, MonitoringState, ServerLifecycle } from "./contracts.gen";
import type { WorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";

import { liveStatus, defaultPolicy, type LiveServer } from "./live-model";
export { activeMap, liveStatus } from "./live-model";
export function MonitoringHub({state,monitoring,client,reload,remove,open,openSettings}:{state:MapState;monitoring:MonitoringState|null;client:WorkspaceClient;reload:()=>Promise<void>;remove:(id:string)=>void;open:(id:string)=>void;openSettings:()=>void}) {
  const [busy,setBusy]=useState(false); const [failure,setFailure]=useState(""); const [message,setMessage]=useState("");
  async function act(action:()=>Promise<unknown>,message:string) {setBusy(true);setFailure("");try {await action();await reload();setMessage(message);}catch(e){setFailure(errorMessage(e));}finally{setBusy(false);}}
  return <div className="monitoring-hub"><div className="section-heading"><div><h2>Servers & monitoring</h2><p>Checks run while InfraWerf is open. Saved inventory remains available during outages.</p></div></div>
    <p className="muted">History retention and SSH alias discovery apply to the whole application. <button className="text-button" onClick={openSettings}>Open Settings</button></p>
    {failure && <p className="form-error" role="alert">{failure}</p>}{message && <p role="status">{message}</p>}
    {state.servers.map(server=>{const record=monitoring?.records.find(r=>r.server_id===server.id);const status=liveStatus({...server,monitoring:record} as LiveServer);return <details className="managed-server" key={server.id}><summary><strong>{server.name}</strong><span>{server.host}</span><span className={`live-status ${status.kind}`} title={status.detail}>{status.label}</span></summary>
      <p>{record?.detail ?? "No monitoring configured"}</p><p className="muted">Last check: {record?.reachability.last_checked_at ? new Date(record.reachability.last_checked_at).toLocaleString() : "Never"} · Last reachable: {record?.reachability.last_success_at ? new Date(record.reachability.last_success_at).toLocaleString() : "Never"}</p>
      {record?.inventory_checked_at && <p className="muted">{record.inventory_detail} · {new Date(record.inventory_checked_at).toLocaleString()}</p>}
      <MonitoringForm key={record?.revision ?? "new"} policy={record?.policy ?? defaultPolicy} busy={busy || !monitoring} save={policy=>void act(()=>client.configureMonitoring(server.id,policy),"Server monitoring settings saved.")} />
      {record?.pending_fingerprint && record.fingerprint && <form className="identity-review" onSubmit={e=>{e.preventDefault();void act(()=>client.acceptMonitoringIdentity(server.id,record.fingerprint!,record.pending_fingerprint!),"Reviewed identity accepted.");}}><h3>Review changed server identity</h3><p>Previous key: <Secret label="Previous host key" value={record.fingerprint} /></p><p>Observed key: <Secret label="Observed host key" value={record.pending_fingerprint} /></p><p>Verify this key with the server owner and update your OpenSSH trust independently. InfraWerf will verify trusted SSH access again before accepting it.</p><label className="check-label"><input type="checkbox" required/> I verified this replacement key belongs to the same server.</label><button disabled={busy || !record.policy.enabled || record.policy.owner_offline}>Accept reviewed identity</button></form>}
      <div className="inspector-actions"><button disabled={busy} onClick={()=>open(server.id)}>Open saved workspace</button><button className="danger text-button" disabled={busy} onClick={()=>remove(server.id)}>Delete local record…</button></div>
      <p className="muted">Archive hides this server from the map and retains history. Ignore also prevents automatic checks and collection. Restore by choosing Active. Deleting removes local inventory, relationships and history; a configured alias may be discovered again.</p>
    </details>;})}
  </div>;
}
function MonitoringForm({policy,busy,save}:{policy:MonitoringPolicy;busy:boolean;save:(policy:MonitoringPolicy)=>void}) {
  const [value,setValue]=useState(policy);
  return <form className="monitoring-form" onSubmit={e=>{e.preventDefault();save(value);}}><label>Lifecycle<select aria-label="Server lifecycle" value={value.lifecycle} disabled={busy} onChange={e=>setValue({...value,lifecycle:e.target.value as ServerLifecycle})}>{(["candidate","active","archived","ignored"] as const).map(v=><option key={v} value={v}>{v[0].toUpperCase()+v.slice(1)}</option>)}</select></label>
    <label className="check-label"><input type="checkbox" checked={value.owner_offline} disabled={busy} onChange={e=>setValue({...value,owner_offline:e.target.checked})}/>Owner-reported offline · do not contact</label>
    <label className="check-label"><input type="checkbox" checked={value.enabled} disabled={busy || value.owner_offline || value.lifecycle!=="active"} onChange={e=>setValue({...value,enabled:e.target.checked})}/>Check reachability and SSH access automatically</label>
    <label className="check-label"><input type="checkbox" checked={value.inventory_enabled} disabled={busy || !value.enabled || value.owner_offline || value.lifecycle!=="active"} onChange={e=>setValue({...value,inventory_enabled:e.target.checked})}/>Collect approved host, service and container inventory</label>
    <div className="monitoring-intervals"><label>Check every (seconds)<input type="number" min={30} max={3600} required value={value.interval_seconds} onChange={e=>setValue({...value,interval_seconds:Number(e.target.value)})}/></label><label>Inventory every (seconds)<input type="number" min={300} max={86400} required value={value.inventory_interval_seconds} onChange={e=>setValue({...value,inventory_interval_seconds:Number(e.target.value)})}/></label></div>
    <p className="muted">Uses existing OpenSSH trust and credentials. A changed or unknown host key requires review before collection. Nothing is installed on the server. Saving waits for the current bounded check to finish before applying your settings.</p><button className="primary" disabled={busy}>{busy?"Finishing current check…":"Save monitoring"}</button>
  </form>;
}
