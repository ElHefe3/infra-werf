import type { MapServer, MapState, MonitoringPolicy, MonitoringRecord, MonitoringState } from "./contracts.gen.ts";
export type LiveServer=MapServer & { monitoring?:MonitoringRecord };
export const defaultPolicy:MonitoringPolicy={lifecycle:"active",enabled:false,owner_offline:false,inventory_enabled:false,interval_seconds:60,inventory_interval_seconds:900};
export function liveStatus(server:LiveServer, now=Date.now()):{label:string;kind:string;detail:string} {
  const record=(server as LiveServer).monitoring;
  if (record?.policy.owner_offline) return {label:"Offline · owner reported",kind:"offline",detail:"Checks paused by owner. Saved workloads are historical."};
  if (record && record.policy.lifecycle!=="active") return {label:record.policy.lifecycle,kind:"paused",detail:"Checks paused"};
  if (record && record.host!==server.host) return {label:"Address changed · review settings",kind:"unknown",detail:"Save monitoring settings to review the new endpoint. The previous identity remains pinned."};
  if (record && !record.policy.enabled) return {label:"Not monitored",kind:"unknown",detail:"Enable monitoring in Servers"};
  const reachability=record?.reachability ?? server.reachability;
  const checked=reachability.last_checked_at;
  if (!checked) return {label:record?.policy.enabled ? "Waiting for check" : "Not checked",kind:"unknown",detail:"No current network observation"};
  if (now-Date.parse(checked)>Math.max(120,(record?.policy.interval_seconds ?? 60)*3)*1000) return {label:"Status stale",kind:"stale",detail:`Last check: ${new Date(checked).toLocaleString()}`};
  const attention=reachability.state==="reachable" && record && record.detail!=="Reachable · SSH verified";
  return {label:attention?"Reachable · SSH needs review":reachability.state==="reachable"?"Reachable":reachability.state==="unreachable"?"Unreachable":"Unknown",kind:attention?"stale":reachability.state,detail:`${record?.detail ?? "Network observation"} · ${new Date(checked).toLocaleString()}`};
}
export function activeMap(state:MapState,monitoring:MonitoringState|null):MapState {
  const servers=state.servers.map(s=>({...s,monitoring:monitoring?.records.find(r=>r.server_id===s.id)})).filter(s=>!s.monitoring || s.monitoring.policy.lifecycle==="active");
  const ids=new Set(servers.map(s=>s.id));
  return {...state,servers,edges:state.edges.filter(e=>ids.has(e.source)&&ids.has(e.target))};
}
