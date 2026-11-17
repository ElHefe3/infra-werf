import test from "node:test";
import assert from "node:assert/strict";
import { badgeLabel, badgesFor, edgeLook, kindsBetween, packWorkloads } from "./pack-map-model.ts";
import type { PackBadgeView, PackCommandView, PackRelationshipKindView, Relationship } from "./contracts.gen.ts";

const badge = (state: PackBadgeView["state"], value: string | null, observed_at: string | null = "2026-11-17T08:00:00Z"): PackBadgeView =>
  ({ server_id: "s1", plugin_id: "org.example.homelab-map", plugin_name: "Homelab map", contribution_id: "state-badge", title: "System state", state, value, observed_at });

test("a badge reads as one of five states and a stale one keeps its old value visible", () => {
  assert.equal(badgeLabel(badge("healthy", "running")).text, "System state: Healthy · running");
  assert.equal(badgeLabel(badge("unhealthy", "degraded")).text, "System state: Unhealthy · degraded");
  assert.equal(badgeLabel(badge("value", "6.8.0")).text, "System state: 6.8.0");
  const stale = badgeLabel(badge("stale", "running"));
  assert.equal(stale.text, "System state: Stale (running)");
  assert.match(stale.detail, /older than 15 minutes/);
  const unknown = badgeLabel(badge("unknown", null, null));
  assert.equal(unknown.text, "System state: Unknown");
  assert.match(unknown.detail, /Not read since InfraWerf started/);
  assert.match(unknown.detail, /Kept in memory only/);
  assert.deepEqual(badgesFor([badge("unknown", null), { ...badge("healthy", "running"), server_id: "s2" }], "s2").map(b => b.state), ["healthy"]);
});

const kind: PackRelationshipKindView = { plugin_id: "org.example.homelab-map", plugin_name: "Homelab map", contribution_id: "backs-up-to", title: "Backs up to", line: "dashed", color: "blue", server_ids: ["a", "b"] };
const edge = (extra: Partial<Relationship> = {}): Relationship => ({ id: "e1", source: "a", target: "b", label: "nightly", kind: null, ...extra });

test("a relationship kind draws its own line, and falls back to plain when its plugin is gone", () => {
  assert.deepEqual(edgeLook(edge(), [kind]), { color: "#89a28e", dash: undefined, label: "nightly" });
  const ref = { plugin_id: kind.plugin_id, kind_id: kind.contribution_id, title: "Backs up to" };
  assert.deepEqual(edgeLook(edge({ kind: ref }), [kind]), { color: "#5b84b1", dash: "7 5", label: "Backs up to: nightly" });
  assert.deepEqual(edgeLook(edge({ kind: ref, label: "" }), []), { color: "#89a28e", dash: undefined, label: "Backs up to" });
});

test("a kind is offered only between two servers its plugin is allowed on", () => {
  assert.deepEqual(kindsBetween([kind], "a", "b").map(k => k.title), ["Backs up to"]);
  assert.deepEqual(kindsBetween([kind], "a", "c"), []);
});

test("a collector's table becomes workloads of the pack's kind", () => {
  const command = (rows: string[][] | null): PackCommandView => ({
    plugin_id: "p", plugin_name: "P", contribution_id: "sockets", title: "Socket units", health_check: false, argv: ["systemctl"], parameters: [],
    workload_kind: { name_column: "unit", state_column: "active" },
    observation: rows && { observed_at: "2026-11-17T08:00:00Z", fresh: true, source: "p.sockets", source_binding: "b", parameters: {}, result: { outcome: "ok", value: null, columns: ["unit", "load", "active"], rows, truncated: false, health: null } },
  });
  assert.deepEqual(packWorkloads(command([["dbus.socket", "loaded", "active"], ["ssh.socket", "loaded", "inactive"]])), [{ name: "dbus.socket", state: "active" }, { name: "ssh.socket", state: "inactive" }]);
  assert.deepEqual(packWorkloads(command(null)), []);
  assert.equal(packWorkloads({ ...command(null), workload_kind: null }), null);
});
