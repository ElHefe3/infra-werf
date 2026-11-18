import test from "node:test";
import assert from "node:assert/strict";
import { installRequest, signerText, updateVerdict, describeChange } from "./plugin-install-model.ts";
import type { UpdateDiff } from "./contracts.gen.ts";

test("the install form sends only what the owner entered, and only https addresses", () => {
  assert.deepEqual(installRequest("archive", " C:/p.zip ", "", ""), { source: { kind: "archive", path: "C:/p.zip" } });
  assert.deepEqual(installRequest("address", "https://example.org/p.zip", "A".repeat(64), " RWkey "), { source: { kind: "address", url: "https://example.org/p.zip" }, expected_sha256: "A".repeat(64), public_key: "RWkey" });
  assert.deepEqual(installRequest("folder", "C:/plugin", "", ""), { source: { kind: "folder", path: "C:/plugin" } });
  assert.match(installRequest("address", "http://example.org/p.zip", "", "") as string, /https/);
  assert.match(installRequest("address", "", "", "") as string, /https/);
  assert.match(installRequest("archive", "", "", "") as string, /Choose/);
  assert.match(installRequest("archive", "C:/p.zip", "abc", "") as string, /64 hexadecimal/);
});

const diff = (extra: Partial<UpdateDiff> = {}): UpdateDiff => ({ from_version: "1.0.0", to_version: "1.1.0", downgrade: false, added_capabilities: [], removed_capabilities: [], contributions: [], needs_review: false, previous_signer: null, signer: null, content_sha256: "a".repeat(64), archive_sha256: null, ...extra });

test("an update says whether it keeps working and what happened to the signature", () => {
  assert.match(updateVerdict(diff()), /keeps working/);
  assert.match(updateVerdict(diff({ needs_review: true })), /switches the plugin off/);
  assert.equal(signerText(diff({ signer: "AB", previous_signer: "AB" })), "Signed by the same key as before (AB).");
  assert.match(signerText(diff({ signer: "CD", previous_signer: "AB" })), /different key \(CD\).*\(AB\)/);
  assert.match(signerText(diff({ previous_signer: "AB" })), /Not checked.*AB/);
  assert.equal(describeChange({ id: "k", title: "Kernel", change: "command_changed", command: "uname -a" }), "Command changed: Kernel");
});
