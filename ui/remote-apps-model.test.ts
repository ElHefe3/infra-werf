import test from "node:test";
import assert from "node:assert/strict";
import { loadShortcuts, parseTarget, saveShortcuts } from "./remote-apps-model.ts";
import { createWorkspaceClient } from "./workspace-client.ts";

test("remote application targets accept only a valid port and relative path", () => {
  assert.deepEqual(parseTarget("3000", ""), { port: 3000, path: "/" });
  assert.deepEqual(parseTarget(" 8080 ", "/admin?x=1"), { port: 8080, path: "/admin?x=1" });
  for (const [port, path] of [["0", "/"], ["65536", "/"], ["80a", "/"], ["", "/"], ["80", "admin"], ["80", "//evil.example/"], ["80", "/a b"], ["80", "/a\\b"]])
    assert.equal(parseTarget(port, path), null, `${port} ${path}`);
});

test("shortcuts persist name, port and path only and ignore corrupt storage", () => {
  const store = new Map<string, string>();
  const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v) };
  saveShortcuts("s1", [{ name: "Grafana", port: 3000, path: "/" }], storage);
  assert.deepEqual(loadShortcuts("s1", storage), [{ name: "Grafana", port: 3000, path: "/" }]);
  assert.deepEqual(loadShortcuts("s2", storage), []);
  store.set("infrawerf.apps.s3", JSON.stringify([{ name: "x", port: 99999, path: "/" }, { name: "", port: 80, path: "/" }, { name: "ok", port: 80, path: "/", password: "no" }]));
  assert.deepEqual(loadShortcuts("s3", storage), [{ name: "ok", port: 80, path: "/" }]);
  store.set("infrawerf.apps.s4", "{broken");
  assert.deepEqual(loadShortcuts("s4", storage), []);
  assert.deepEqual(loadShortcuts("s1", { getItem: () => { throw new Error("blocked"); } }), []);
});

test("tunnel client sends only server ID, port and path", async () => {
  const calls: [string, unknown][] = [];
  const client = createWorkspaceClient(async (command, args) => { calls.push([command, args]); return { contract_version: 1 } as never; });
  await client.openTunnel("server-1", { remote_port: 3000, path: "/" }).catch(() => {});
  await client.closeTunnel("t1");
  assert.deepEqual(calls[0], ["tunnel_open", { serverId: "server-1", request: { remote_port: 3000, path: "/" } }]);
  assert.deepEqual(calls[1], ["tunnel_close", { tunnelId: "t1" }]);
});

test("discovered applications are labelled by title, process or port and only HTTP ones open", async () => {
  const { describeApp, appKey } = await import("./remote-apps-model.ts");
  const base = { bind: "127.0.0.1", remote_host: null, http_status: 200, https_only: false, title: null, process: null };
  assert.deepEqual(describeApp({ ...base, port: 3000, title: "Grafana" }), { label: "Grafana — localhost:3000", openable: true });
  assert.deepEqual(describeApp({ ...base, port: 8080, process: "java", remote_host: "192.168.20.12" }), { label: "java — 192.168.20.12:8080", openable: true });
  assert.equal(describeApp({ ...base, port: 22, http_status: null }).openable, false);
  assert.equal(describeApp({ ...base, port: 9443, http_status: null, https_only: true }).label, "Port 9443 — localhost:9443 (HTTPS only, not supported yet)");
  assert.equal(appKey({ port: 80, remote_host: null }), "localhost:80");
  assert.equal(appKey({ port: 80, remote_host: "10.0.0.2" }), "10.0.0.2:80");
});

test("discovery client sends only the server ID", async () => {
  const calls: [string, unknown][] = [];
  const client = createWorkspaceClient(async (command, args) => { calls.push([command, args]); return { contract_version: 1 } as never; });
  await client.discoverApps("server-1").catch(() => {});
  assert.deepEqual(calls[0], ["tunnel_discover", { serverId: "server-1" }]);
});
