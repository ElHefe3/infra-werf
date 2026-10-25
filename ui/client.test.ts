import test from "node:test";
import assert from "node:assert/strict";
import { createNativeClient, createCoordinatorClient } from "./client.ts";
import { CONTRACT_VERSION, type FoundationStatus } from "./contracts.gen.ts";

for (const [name, factory] of [["native", createNativeClient], ["coordinator", createCoordinatorClient]] as const) {
  test(`${name} adapter accepts its shared version and rejects an incompatible transport`, async () => {
    const status: FoundationStatus = { contract_version: CONTRACT_VERSION, version: "fixture", platform: "fixture", phase: "contract test" };
    assert.deepEqual(await factory(async () => status).status(), status);
    await assert.rejects(factory(async () => ({ ...status, contract_version: CONTRACT_VERSION + 1 })).status(), /incompatible/);
  });
}
