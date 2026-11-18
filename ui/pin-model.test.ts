import test from "node:test";
import assert from "node:assert/strict";
import { mask, pinAware, setPinPrompt, validPin } from "./pin-model.ts";

test("a pin_required refusal asks once and retries only after an unlock", async () => {
  let calls = 0, asked = 0;
  const transport = (async () => { calls++; if (calls === 1) throw { code: "pin_required" }; return "ok"; }) as never;
  setPinPrompt(async () => { asked++; return true; });
  assert.equal(await pinAware(transport)("files_open"), "ok");
  assert.deepEqual([calls, asked], [2, 1]);
  calls = 0; setPinPrompt(async () => false);
  await assert.rejects(pinAware(transport)("files_open"), { code: "pin_required" });
  assert.equal(calls, 1);
  calls = 1; setPinPrompt(null);
  const other = (async () => { throw { code: "denied" }; }) as never;
  await assert.rejects(pinAware(other)("x"), { code: "denied" });
});

test("PINs are 4 to 12 digits and masks keep only a hint", () => {
  assert.ok(validPin("1234") && validPin("123456789012"));
  assert.ok(!validPin("123") && !validPin("12a4") && !validPin("1234567890123"));
  assert.equal(mask("SHA256:abcdef"), "••••••••ef");
  assert.equal(mask("ab"), "••••");
});
