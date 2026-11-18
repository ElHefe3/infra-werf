import test from "node:test";
import assert from "node:assert/strict";
import { describeJobRun, scheduleLabel } from "./plugin-jobs-model.ts";
import type { JobRunView } from "./contracts.gen.ts";

const run = (extra: Partial<JobRunView>): JobRunView => ({ run_id: "r", plugin_id: "p", contribution_id: "c", server_id: "s", attended: true, started_at: "2026-11-17T10:00:00Z", finished_at: null, state: "running", outcome: null, actions: [], ...extra });
const outcome = (extra: object) => ({ exit_code: null, stopped: null, reason: null, logs: [], result: null, fuel_used: 0, host_calls: 0, refused: [], ...extra });

test("a run's end is told in words", () => {
  assert.equal(describeJobRun(run({})), "Running…");
  assert.equal(describeJobRun(run({ state: "done", outcome: outcome({ exit_code: 0 }) })), "Finished");
  assert.equal(describeJobRun(run({ state: "failed", outcome: outcome({ stopped: "out_of_fuel" }) })), "Failed: the job used all its time");
  assert.equal(describeJobRun(run({ state: "failed", outcome: outcome({ exit_code: 3 }) })), "Failed: exit code 3");
  assert.match(describeJobRun(run({ state: "cancelled" })), /Cancelled/);
});

test("a schedule says when it runs, or that it paused", () => {
  assert.match(scheduleLabel(null), /only when you press Run/);
  assert.match(scheduleLabel({ every_minutes: 15, next_run_at: "2026-11-17T10:15:00Z", last_run_at: null, last_outcome: "done", failures: 0, paused: false }), /^Every 15 minutes while InfraWerf is open · next at .* · last run done$/);
  assert.match(scheduleLabel({ every_minutes: 5, next_run_at: "2026-11-17T10:15:00Z", last_run_at: null, last_outcome: "failed", failures: 3, paused: true }), /paused after 3 failed runs/);
});
