import type { KindColor, LineStyle, PackBadgeState, PackBadgeView, PackCommandView, PackRelationshipKindView, Relationship } from "./contracts.gen.ts";

// What plugin packs add to existing screens (F21/T044). The core decides
// which badges, kinds and applications exist; this file only says how each
// one reads and looks, from a fixed set a pack cannot extend.

const STATE: Record<PackBadgeState, string> = { healthy: "Healthy", unhealthy: "Unhealthy", value: "", stale: "Stale", unknown: "Unknown" };

/** A badge's visible text and its tooltip. */
export function badgeLabel(badge: PackBadgeView): { text: string; detail: string } {
  const value = badge.value ?? "";
  const word = STATE[badge.state];
  const text = badge.state === "value" ? `${badge.title}: ${value}`
    : badge.state === "stale" ? `${badge.title}: ${word}${value ? ` (${value})` : ""}`
    : `${badge.title}: ${word}${value && badge.state !== "unknown" ? ` · ${value}` : ""}`;
  const when = badge.observed_at ? `Read ${new Date(badge.observed_at).toLocaleString()}` : "Not read since InfraWerf started. Run it from the server's Plugin data tab.";
  const age = badge.state === "stale" ? " · older than 15 minutes, run it again for a current answer" : "";
  return { text, detail: `${when}${age} · From ${badge.plugin_name} · Kept in memory only` };
}

export const badgesFor = (badges: PackBadgeView[], serverId: string) => badges.filter(badge => badge.server_id === serverId);

const COLOR: Record<KindColor, string> = { green: "#89a28e", blue: "#5b84b1", amber: "#c2913a", red: "#b5604f", purple: "#8a6fb0", grey: "#8f978c" };
const DASH: Record<LineStyle, string | undefined> = { solid: undefined, dashed: "7 5", dotted: "1.5 4" };

/** How a map relationship is drawn, and its label. A kind from a plugin that is no longer enabled keeps its title and falls back to the plain line. */
export function edgeLook(edge: Relationship, kinds: PackRelationshipKindView[]): { color: string; dash: string | undefined; label: string } {
  const kind = edge.kind ? kinds.find(k => k.plugin_id === edge.kind!.plugin_id && k.contribution_id === edge.kind!.kind_id) : undefined;
  const title = edge.kind?.title;
  const label = title ? (edge.label ? `${title}: ${edge.label}` : title) : edge.label;
  return { color: COLOR[kind?.color ?? "green"], dash: kind ? DASH[kind.line] : undefined, label };
}

/** Kinds that may be chosen between two servers: the plugin is allowed on both. */
export const kindsBetween = (kinds: PackRelationshipKindView[], source: string, target: string) =>
  kinds.filter(kind => kind.server_ids.includes(source) && kind.server_ids.includes(target));

export const kindKey = (kind: Pick<PackRelationshipKindView, "plugin_id" | "contribution_id">) => `${kind.plugin_id}/${kind.contribution_id}`;

/** A collector's table rows as workloads of the kind its pack names. */
export function packWorkloads(command: PackCommandView): { name: string; state: string }[] | null {
  const kind = command.workload_kind, result = command.observation?.result;
  if (!kind || !result || result.outcome !== "ok") return kind ? [] : null;
  const name = result.columns.indexOf(kind.name_column);
  const state = kind.state_column ? result.columns.indexOf(kind.state_column) : -1;
  if (name < 0) return [];
  return result.rows.map(row => ({ name: row[name] ?? "", state: state >= 0 ? row[state] ?? "" : "" })).filter(row => row.name);
}
