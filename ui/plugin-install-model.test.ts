import test from "node:test";
import assert from "node:assert/strict";
import { installRequest } from "./plugin-install-model.ts";

test("the install form sends only what the owner entered, and only https addresses", () => {
  assert.deepEqual(installRequest("archive", " C:/p.zip ", "", ""), { source: { kind: "archive", path: "C:/p.zip" } });
  assert.deepEqual(installRequest("address", "https://example.org/p.zip", "A".repeat(64), " RWkey "), { source: { kind: "address", url: "https://example.org/p.zip" }, expected_sha256: "A".repeat(64), public_key: "RWkey" });
  assert.deepEqual(installRequest("folder", "C:/plugin", "", ""), { source: { kind: "folder", path: "C:/plugin" } });
  assert.match(installRequest("address", "http://example.org/p.zip", "", "") as string, /https/);
  assert.match(installRequest("address", "", "", "") as string, /https/);
  assert.match(installRequest("archive", "", "", "") as string, /Choose/);
  assert.match(installRequest("archive", "C:/p.zip", "abc", "") as string, /64 hexadecimal/);
});
