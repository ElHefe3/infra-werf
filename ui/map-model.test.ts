import test from "node:test";
import assert from "node:assert/strict";
import { errorMessage, inventoryRows, matches, reachabilityLabel, workloads } from "./map-model.ts";
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

test("structured SSH errors explain trust, authentication and session failures", () => {
  assert.match(errorMessage({code: "host_trust_failure"}), /host key.*trust files/);
  assert.match(errorMessage({code: "unauthenticated"}), /SSH identity/);
  assert.match(errorMessage({code: "expired_session"}), /Reconnect/);
});

test("relationship lines get their own connection points on facing sides", async () => {
  const { routeEdges } = await import("./map-model.ts");
  const servers = [{ id: "dns", x: 0, y: 0 }, { id: "a", x: 600, y: -300 }, { id: "b", x: 600, y: 0 }, { id: "c", x: 600, y: 300 }, { id: "d", x: 0, y: 600 }];
  const edges = ["a", "b", "c", "d"].map(target => ({ id: `e-${target}`, source: "dns", target }));
  const { ports, routes } = routeEdges(servers, edges);
  // Three servers to the right share the right side, in top-to-bottom order; the one below uses the bottom.
  const right = ports.get("dns")!.filter(p => p.side === "right");
  assert.deepEqual(right.map(p => p.id), ["right-source-e-a", "right-source-e-b", "right-source-e-c"]);
  assert.ok(right[0].offset < right[1].offset && right[1].offset < right[2].offset);
  assert.equal(routes.get("e-d")?.sourceHandle, "bottom-source-e-d");
  assert.equal(routes.get("e-d")?.targetHandle, "top-target-e-d");
  assert.equal(routes.get("e-a")?.targetHandle, "left-target-e-a");
  assert.equal(new Set([...ports.values()].flat().map(p => p.id)).size, 8);
  // A line to a missing server or to itself is left to the default.
  assert.equal(routeEdges(servers, [{ id: "x", source: "dns", target: "gone" }, { id: "y", source: "a", target: "a" }]).routes.size, 0);
});

test("labels move along their own line until nothing covers them", async () => {
  const { pathPoints, pointAlong, placeLabels } = await import("./map-model.ts");
  const points = pathPoints("M 0,0 L 90,0Q 100,0 100,10 L 100,100");
  assert.deepEqual(points, [{ x: 0, y: 0 }, { x: 90, y: 0 }, { x: 100, y: 10 }, { x: 100, y: 100 }]);
  assert.deepEqual(pointAlong([{ x: 0, y: 0 }, { x: 100, y: 0 }], 0.25), { x: 25, y: 0 });
  // Two lines on top of each other: the second label slides away from the first.
  const line = [{ x: 0, y: 0 }, { x: 400, y: 0 }];
  const places = placeLabels([{ id: "a", points: line, width: 80, height: 20 }, { id: "b", points: line, width: 80, height: 20 }], []);
  assert.deepEqual(places.get("a"), { x: 200, y: 0 });
  assert.ok(Math.abs(places.get("b")!.x - 200) >= 80);
  // A card over the middle pushes the label off it.
  const card = [{ x: 150, y: -50, width: 100, height: 100 }];
  const clear = placeLabels([{ id: "c", points: line, width: 60, height: 20 }], card).get("c")!;
  assert.ok(clear.x + 30 < 150 || clear.x - 30 > 250);
});
