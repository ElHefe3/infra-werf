import test from "node:test";
import assert from "node:assert/strict";
import { createNativeClient, createCoordinatorClient } from "./client.ts";
import { CONTRACT_VERSION, type FoundationStatus, type WorkloadControlPreview, type WorkloadControlReceipt } from "./contracts.gen.ts";
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

test("workload control sends only reviewed action identity and one-use operation ID", async () => {
  const calls: Array<[string, Record<string, unknown> | undefined]> = [];
  const workload = { id: "wl1_fixture", kind: "service", scope: "user", runtime: "systemd", name: "worker.service", reported_state: "stopped", reported_detail: "dead", supported: true } as const;
  const preview: WorkloadControlPreview = { contract_version: CONTRACT_VERSION, operation_id: "operation-a", expires_at: "2026-10-26T10:00:30Z", server: "atlas-linux", endpoint: "fixture@127.0.0.1:2222", workload, action: "start", requires_sudo: false };
  const receipt: WorkloadControlReceipt = { contract_version: CONTRACT_VERSION, operation_id: "operation-a", workload_id: workload.id, action: "start", outcome: "completed", observed_state: "running", observed_at: "2026-10-26T10:00:01Z", message: "Running was observed after the start request." };
  const client = createWorkspaceClient(async (command, args) => {
    calls.push([command, args]);
    return (command === "workload_control_preview" ? preview : receipt) as never;
  });
  assert.equal((await client.previewWorkloadControl("server-a", "session-a", workload.id, "start")).requires_sudo, false);
  assert.equal((await client.confirmWorkloadControl("server-a", "session-a", "operation-a")).outcome, "completed");
  assert.deepEqual(calls, [
    ["workload_control_preview", { serverId: "server-a", sessionId: "session-a", workloadId: workload.id, action: "start" }],
    ["workload_control_confirm", { serverId: "server-a", sessionId: "session-a", operationId: "operation-a", password: undefined }],
  ]);
});
