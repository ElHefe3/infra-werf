import test from "node:test";
import assert from "node:assert/strict";
import { createNativeClient, createCoordinatorClient } from "./client.ts";
import { CONTRACT_VERSION, type FoundationStatus } from "./contracts.gen.ts";
import { createWorkspaceClient } from "./workspace-client.ts";

for (const [name, factory] of [["native", createNativeClient], ["coordinator", createCoordinatorClient]] as const) {
  test(`${name} adapter accepts its shared version and rejects an incompatible transport`, async () => {
    const status: FoundationStatus = { contract_version: CONTRACT_VERSION, version: "fixture", platform: "fixture", phase: "contract test" };
    assert.deepEqual(await factory(async () => status).status(), status);
    await assert.rejects(factory(async () => ({ ...status, contract_version: CONTRACT_VERSION + 1 })).status(), /incompatible/);
  });
}

test("protected workload inspection sends only host, session and opaque workload ID", async () => {
  const calls: Array<[string, Record<string, unknown> | undefined]> = [];
  const client = createWorkspaceClient(async (command, args) => {
    calls.push([command, args]);
    return {
      contract_version: CONTRACT_VERSION,
      workload_id: "wl1_fixture",
      inspected_at: "2026-10-26T10:00:00Z",
      state: "running",
      details: [],
      logs_state: "available",
      logs: "synthetic protected output",
      logs_truncated: false,
    } as never;
  });
  const result = await client.inspectWorkload("server-a", "session-a", "wl1_fixture");
  assert.equal(result.workload_id, "wl1_fixture");
  assert.deepEqual(calls, [["workload_inspect", { serverId: "server-a", sessionId: "session-a", workloadId: "wl1_fixture" }]]);
});
