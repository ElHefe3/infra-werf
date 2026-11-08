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
