import test from "node:test";
import assert from "node:assert/strict";
import { inventoryRows, matches, reachabilityLabel, workloads } from "./map-model.ts";
import { createWorkspaceClient, type NativeTransport } from "./workspace-client.ts";
import { CONTRACT_VERSION, type HealthProbeConfig, type HealthSnapshot, type MapServer, type MapState } from "./contracts.gen.ts";

const server: MapServer = { id: "host-a", name: "Runner", host: "runner.invalid", x: 45, y: 65, created_at: "2026-01-01T00:00:00Z", freshness: "stale", reachability: { state: "unknown", last_checked_at: null, last_success_at: null, last_failure_at: null }, inventory: { hostname: "worker", collected_at: "2026-01-01T00:00:00Z", source_host: "runner.invalid", source_binding: null, cpus: 4, memory_total_kib: 1024, containers: [{ scope: "user", engine: "podman", name: "api", image: "example:v1", state: "running" }], services: [{ scope: "user", name: "api", state: "active", substate: "running" }, { scope: "system", name: "api", state: "inactive", substate: "dead" }] } };
const state: MapState = { contract_version: 1, servers: [server], edges: [], events: [], served_at: "2026-01-01T00:00:00Z" };

test("nested workload identity includes host, scope and kind; search reaches full inventory", () => {
  assert.equal(new Set(workloads(server).map(w => w.id)).size, 3);
  assert.ok(matches(server, "EXAMPLE:v1"));
  assert.ok(matches(server, "worker"));
  assert.equal(matches(server, "missing"), false);
  const large = structuredClone(state);
  large.servers[0].inventory!.services = Array.from({ length: 540 }, (_, i) => ({ scope: "system", name: `unit-${i}`, state: "inactive", substate: "dead" }));
  assert.equal(inventoryRows(large, "unit-539")[0].workload.name, "unit-539");
  assert.equal(inventoryRows(large, "").length, 541);
});

test("native map mutations send only typed operation arguments and reject incompatible results", async () => {
  const calls: unknown[] = [];
  const transport: NativeTransport = async <T>(command: string, args?: Record<string, unknown>) => { calls.push({ command, args }); return state as T; };
  const client = createWorkspaceClient(transport);
  await client.update(server.id, { x: 150, y: 250 });
  await client.connect({ source: "host-a", target: "host-b", label: "Backs up" });
  assert.deepEqual(calls, [{ command: "update_server", args: { id: "host-a", input: { x: 150, y: 250 } } }, { command: "add_relationship", args: { input: { source: "host-a", target: "host-b", label: "Backs up" } } }]);
  await assert.rejects(createWorkspaceClient(async <T>() => ({ ...state, contract_version: 99 }) as T).load(), /incompatible/);
});

test("reachability stays unknown, fresh or stale without replacing old observations", () => {
  assert.equal(reachabilityLabel(server, Date.parse("2026-01-01T00:20:00Z")), "Unknown · not checked");
  const observed = { ...server, reachability: { state: "reachable" as const, last_checked_at: "2026-01-01T00:10:00Z", last_success_at: "2026-01-01T00:10:00Z", last_failure_at: null } };
  assert.equal(reachabilityLabel(observed, Date.parse("2026-01-01T00:15:00Z")), "reachable");
  assert.equal(reachabilityLabel(observed, Date.parse("2026-01-01T00:26:00Z")), "Stale · reachable");
});

test("health client uses the versioned desktop commands and exact configured endpoint", async () => {
  const calls: unknown[] = [];
  const config: HealthProbeConfig = { server_id: server.id, host: server.host, host_port: 22, service_port: 8080, service_kind: "http", path: "/health", interval_seconds: 60 };
  const snapshot: HealthSnapshot = {
    contract_version: CONTRACT_VERSION,
    server_id: server.id,
    host: server.host,
    host_reachability: { state: "reachable", last_checked_at: "2026-10-26T10:00:00Z", last_success_at: "2026-10-26T10:00:00Z", last_failure_at: null },
    service: { state: "unhealthy", last_checked_at: "2026-10-26T10:00:00Z", last_success_at: null, last_failure_at: "2026-10-26T10:00:00Z" },
    observed_at: "2026-10-26T10:00:00Z",
  };
  const client = createWorkspaceClient(async <T>(command: string, args?: Record<string, unknown>) => {
    calls.push({ command, args });
    return (command === "health_cancel" ? undefined : snapshot) as T;
  });
  assert.deepEqual(await client.configureHealth(server.id, config), snapshot);
  assert.deepEqual(await client.healthStatus(server.id), snapshot);
  await client.cancelHealth(server.id);
  assert.deepEqual(calls, [
    { command: "health_configure", args: { serverId: server.id, config } },
    { command: "health_status", args: { serverId: server.id } },
    { command: "health_cancel", args: { serverId: server.id } },
  ]);
  await assert.rejects(createWorkspaceClient(async <T>() => ({ ...snapshot, contract_version: CONTRACT_VERSION + 1 }) as T).healthStatus(server.id), /incompatible/);
});
