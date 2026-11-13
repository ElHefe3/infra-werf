import test from "node:test";
import assert from "node:assert/strict";
import { bind, contributionsAt, ExtensionBindingError } from "./extensions-model.ts";
import { createWorkspaceClient, type NativeTransport } from "./workspace-client.ts";
import { CONTRACT_VERSION, type Contribution, type ExtensionCatalogue } from "./contracts.gen.ts";

const entry = (id: string, order: number, point: Contribution["point"] = "workspace_tab", capabilities: Contribution["capabilities"] = ["read_saved_state"]): Contribution =>
  ({ id, point, title: id.split(".").at(-1)!, owner: "builtin", order, capabilities });
const catalogue = (...contributions: Contribution[]): ExtensionCatalogue => ({ contract_version: CONTRACT_VERSION, contributions });

test("contributions are bound in registry order, by point", () => {
  const value = catalogue(entry("builtin.tab.files", 30), entry("builtin.tab.services", 10), entry("builtin.badge.live-status", 5, "server_badge"), entry("builtin.tab.health", 10));
  assert.deepEqual(contributionsAt(value, "workspace_tab").map(c => c.id), ["builtin.tab.health", "builtin.tab.services", "builtin.tab.files"]);
  const bound = bind(value, "workspace_tab", { "builtin.tab.services": "S", "builtin.tab.files": "F", "builtin.tab.health": "H" });
  assert.deepEqual(bound.map(b => b.renderer), ["H", "S", "F"]);
  assert.deepEqual(bind(value, "server_badge", { "builtin.badge.live-status": 1 }).map(b => b.contribution.id), ["builtin.badge.live-status"]);
  assert.deepEqual(bind(value, "settings_section", {}), []);
});

test("a registered contribution without a renderer, or a renderer without a registration, is rejected", () => {
  const value = catalogue(entry("builtin.tab.services", 10), entry("builtin.tab.files", 30));
  assert.throws(() => bind(value, "workspace_tab", { "builtin.tab.services": "S" }), (error: unknown) => error instanceof ExtensionBindingError && /builtin\.tab\.files has no interface renderer/.test(error.message));
  assert.throws(() => bind(value, "workspace_tab", { "builtin.tab.services": "S", "builtin.tab.files": "F", "rogue.tab.extra": "X" }), /rogue\.tab\.extra is not a registered workspace_tab extension/);
  // A renderer for another point's contribution is not accepted either.
  const mixed = catalogue(entry("builtin.tab.services", 10), entry("builtin.badge.live-status", 5, "server_badge"));
  assert.throws(() => bind(mixed, "workspace_tab", { "builtin.tab.services": "S", "builtin.badge.live-status": "B" }), /not a registered workspace_tab extension/);
});

test("duplicate IDs and contributions without capabilities are rejected", () => {
  assert.throws(() => bind(catalogue(entry("builtin.tab.files", 1), entry("builtin.tab.files", 2)), "workspace_tab", { "builtin.tab.files": "F" }), /registered twice/);
  assert.throws(() => bind(catalogue(entry("builtin.tab.files", 1, "workspace_tab", [])), "workspace_tab", { "builtin.tab.files": "F" }), /declares no capabilities/);
});

test("the client reads the catalogue and refuses an incompatible contract", async () => {
  const calls: string[] = [];
  const value = catalogue(entry("builtin.tab.services", 10));
  const transport: NativeTransport = async <T>(command: string) => { calls.push(command); return value as T; };
  assert.deepEqual(await createWorkspaceClient(transport).extensions(), value);
  assert.deepEqual(calls, ["extensions_state"]);
  const stale: NativeTransport = async <T>() => ({ ...value, contract_version: CONTRACT_VERSION + 1 }) as T;
  await assert.rejects(createWorkspaceClient(stale).extensions(), /incompatible/);
});
