import test from "node:test";
import assert from "node:assert/strict";
import { activeMap, defaultPolicy, liveStatus, type LiveServer } from "./live-model.ts";
import type { MonitoringRecord, MapState } from "./contracts.gen.ts";

const now=Date.parse("2026-10-27T10:00:00Z");
const reachability={state:"reachable" as const,last_checked_at:new Date(now).toISOString(),last_success_at:new Date(now).toISOString(),last_failure_at:null};
const record:MonitoringRecord={server_id:"one",host:"node",policy:{...defaultPolicy,enabled:true},reachability,detail:"Reachable \u00b7 SSH verified",fingerprint:null,pending_fingerprint:null,inventory_checked_at:null,inventory_detail:null,revision:"r1"};
const server:LiveServer={id:"one",host:"node",name:"Node",x:0,y:0,created_at:new Date(now).toISOString(),inventory:null,freshness:"unknown",reachability,monitoring:record};
test("current status ages without successful IPC and owner offline overrides saved success",()=>{
  assert.equal(liveStatus(server,now).label,"Reachable");
  assert.equal(liveStatus(server,now+181000).kind,"stale");
  assert.equal(liveStatus({...server,monitoring:{...record,policy:{...record.policy,owner_offline:true}}},now).kind,"offline");
  assert.match(liveStatus({...server,monitoring:{...record,detail:"SSH identity changed; review required"}},now).label,/SSH needs review/);
});
test("archive and ignore filter map and edges while retaining original records",()=>{
  const state:MapState={contract_version:1,servers:[server,{...server,id:"two"}],edges:[{id:"edge",source:"one",target:"two",label:"deploy"}],events:[],served_at:new Date(now).toISOString()};
  for(const lifecycle of ["archived","ignored","candidate"] as const){const visible=activeMap(state,{contract_version:1,records:[{...record,policy:{...record.policy,lifecycle}}],history_limit:1000});assert.equal(visible.servers.length,1);assert.equal(visible.edges.length,0);assert.equal(state.servers.length,2);}
  assert.equal(activeMap(state,{contract_version:1,records:[record],history_limit:1000}).edges.length,1);
});
