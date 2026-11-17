import test from "node:test";
import assert from "node:assert/strict";
import { fileErrors, lateFailure } from "./file-session-model.ts";

// A read starts at generation 4; the closing notice arrives first and clears to 5.
test("a failure whose closing notice arrived first still gives its reason", () => {
  assert.equal(lateFailure(5, 4, 5, "output_limit"), fileErrors.output_limit);
  assert.match(lateFailure(5, 4, 5, "output_limit")!, /safety limit/);
  assert.equal(lateFailure(5, 4, 5, "io_failure"), fileErrors.io_failure);
});

test("a late failure is ignored when nothing was in flight or the view moved on", () => {
  // The notice arrived while no request was running.
  assert.equal(lateFailure(null, 4, 5, "output_limit"), null);
  // The person locked, reconnected or left since: a newer generation owns the view.
  assert.equal(lateFailure(5, 4, 6, "output_limit"), null);
  // A request from before the interrupted one.
  assert.equal(lateFailure(5, 3, 5, "output_limit"), null);
  // An unknown code says nothing more than the closing notice did.
  assert.equal(lateFailure(5, 4, 5, undefined), null);
  assert.equal(lateFailure(5, 4, 5, "cancelled"), null);
});
