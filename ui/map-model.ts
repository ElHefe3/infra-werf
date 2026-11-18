import type { MapServer, MapState } from "./contracts.gen.ts";

export type Workload = { id: string; kind: "service" | "container"; scope: string; engine: string; name: string; state: string; detail: string };

export function workloads(server: MapServer): Workload[] {
  const inventory = server.inventory;
  if (!inventory) return [];
  // Scope and engine are part of identity: user/system services and containers
  // with the same display name remain separate records under their host.
  return [
    ...inventory.services.map(s => ({ id: JSON.stringify([server.id, "service", s.scope, s.name]), kind: "service" as const, engine: "systemd", scope: s.scope, name: s.name, state: s.state, detail: s.substate })),
    ...inventory.containers.map(c => ({ id: JSON.stringify([server.id, "container", c.scope, c.engine, c.name]), kind: "container" as const, engine: c.engine, scope: c.scope, name: c.name, state: c.state, detail: c.image })),
  ];
}

export function matches(server: MapServer, query: string): boolean {
  const q = query.trim().toLocaleLowerCase();
  return !q || [server.name, server.host, server.inventory?.hostname ?? "", ...workloads(server).flatMap(w => [w.name, w.scope, w.engine, w.detail])].some(value => value.toLocaleLowerCase().includes(q));
}

export function inventoryRows(state: MapState, query: string): { server: MapServer; workload: Workload }[] {
  const q = query.trim().toLocaleLowerCase();
  return state.servers.flatMap(server => workloads(server)
    .filter(w => !q || [server.name, server.host, w.name, w.scope, w.engine, w.detail, w.state].some(s => s.toLocaleLowerCase().includes(q)))
    .map(workload => ({ server, workload })));
}

export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error && "code" in error) {
    const messages: Record<string, string> = {
      conflict: "That host or relationship already exists, or another InfraWerf window owns this database. Check the details and try again.",
      invalid_input: "Check the name, host and relationship details. A host must be a hostname or IP address, without a URL or SSH options.",
      not_found: "That item no longer exists. Reload the workspace and try again.",
      migration_failed: "InfraWerf could not import the reference database. Your original was preserved. Check the configured source before retrying.",
      incompatible_version: "This workspace uses an incompatible data version. Use the matching InfraWerf build; do not replace the database.",
      io_failure: "InfraWerf could not open or save its workspace. Check disk access and whether another process is using the database, then retry.",
      denied: "This operation is unavailable in the current workspace.",
      host_trust_failure: "The verified SSH host key could not be matched to the configured trust files. Verify SSH access in Settings and review the host identity before reconnecting.",
      unauthenticated: "The server did not accept the configured SSH identity. Check this PC’s OpenSSH configuration and identity, then reconnect.",
      expired_session: "This SSH session was locked or expired. Reconnect to continue.",
      pin_required: "Enter your InfraWerf PIN to open this.",
      cancelled: "The SSH check was cancelled. No authenticated session is open.",
      timed_out: "The SSH check exceeded its time limit and was stopped. Check the host/VPN connection before retrying.",
      output_limit: "The SSH check produced more output than allowed and was stopped. No partial result was retained.",
    };
    return messages[String(error.code)] ?? "InfraWerf could not complete the operation. Try again.";
  }
  return error instanceof Error ? error.message : "InfraWerf could not complete the operation. Try again.";
}

export function observedLabel(server: MapServer): string {
  return server.freshness === "saved" ? "Recent saved observation" : server.freshness === "stale" ? "Stale observation" : "Not observed";
}

export function reachabilityLabel(server: MapServer, now = Date.now()): string {
  const checked = server.reachability.last_checked_at;
  if (!checked) return "Unknown · not checked";
  const state = server.reachability.state;
  return now - Date.parse(checked) > 15 * 60 * 1000 ? `Stale · ${state}` : state;
}

/** Where a relationship line meets a server card (T063). */
export type Side = "top" | "right" | "bottom" | "left";
export type Port = { id: string; type: "source" | "target"; side: Side; offset: number };
export type Route = { sourceHandle: string; targetHandle: string };
export const CARD = { width: 217, height: 210 };

/**
 * Gives every relationship its own connection point on the side of each card
 * that faces the other server, spread along that side in the order of the far
 * ends, so lines and their labels do not run on top of each other.
 */
export function routeEdges(servers: { id: string; x: number; y: number }[], edges: { id: string; source: string; target: string }[], size = CARD): { ports: Map<string, Port[]>; routes: Map<string, Route> } {
  const centre = new Map(servers.map(s => [s.id, { x: s.x + size.width / 2, y: s.y + size.height / 2 }]));
  const groups = new Map<string, { edge: string; type: Port["type"]; side: Side; node: string; along: number }[]>();
  const opposite: Record<Side, Side> = { top: "bottom", bottom: "top", left: "right", right: "left" };
  for (const edge of edges) {
    const a = centre.get(edge.source), b = centre.get(edge.target);
    if (!a || !b || edge.source === edge.target) continue;
    const dx = b.x - a.x, dy = b.y - a.y;
    const across = Math.abs(dx) / size.width >= Math.abs(dy) / size.height;
    const from: Side = across ? (dx >= 0 ? "right" : "left") : (dy >= 0 ? "bottom" : "top");
    for (const [node, side, type, far] of [[edge.source, from, "source", b], [edge.target, opposite[from], "target", a]] as const) {
      const key = `${node}|${side}`;
      const along = side === "left" || side === "right" ? far.y : far.x;
      groups.set(key, [...(groups.get(key) ?? []), { edge: edge.id, type, side, node, along }]);
    }
  }
  const ports = new Map<string, Port[]>();
  const routes = new Map<string, Partial<Route>>();
  for (const entries of groups.values()) {
    entries.sort((p, q) => p.along - q.along || (p.edge < q.edge ? -1 : 1));
    entries.forEach((entry, index) => {
      const id = `${entry.side}-${entry.type}-${entry.edge}`;
      const offset = 0.15 + 0.7 * (index + 1) / (entries.length + 1);
      ports.set(entry.node, [...(ports.get(entry.node) ?? []), { id, type: entry.type, side: entry.side, offset }]);
      routes.set(entry.edge, { ...routes.get(entry.edge), [entry.type === "source" ? "sourceHandle" : "targetHandle"]: id });
    });
  }
  return { ports, routes: new Map([...routes].filter((r): r is [string, Route] => Boolean(r[1].sourceHandle && r[1].targetHandle))) };
}

export type Point = { x: number; y: number };
export type Rect = { x: number; y: number; width: number; height: number };

/** Corner points of an SVG path made of M, L and Q segments (curves count by their end point). */
export function pathPoints(d: string): Point[] {
  const points: Point[] = [];
  for (const [, command, args] of d.matchAll(/([MLQ])\s*([^MLQ]*)/g)) {
    const numbers = (args.match(/-?\d*\.?\d+(?:e-?\d+)?/gi) ?? []).map(Number);
    const step = command === "Q" ? 4 : 2;
    for (let i = 0; i + step <= numbers.length; i += step) points.push({ x: numbers[i + step - 2], y: numbers[i + step - 1] });
  }
  return points;
}

/** The point a given fraction of the way along a polyline. */
export function pointAlong(points: Point[], fraction: number): Point | null {
  if (!points.length) return null;
  const lengths = points.slice(1).map((p, i) => Math.hypot(p.x - points[i].x, p.y - points[i].y));
  let left = lengths.reduce((a, b) => a + b, 0) * fraction;
  for (let i = 0; i < lengths.length; i++) {
    if (left <= lengths[i] || i === lengths.length - 1) {
      const t = lengths[i] ? Math.min(1, left / lengths[i]) : 0;
      return { x: points[i].x + (points[i + 1].x - points[i].x) * t, y: points[i].y + (points[i + 1].y - points[i].y) * t };
    }
    left -= lengths[i];
  }
  return points[0];
}

const overlap = (a: Rect, b: Rect) => Math.max(0, Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x)) * Math.max(0, Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y));
const FRACTIONS = [0.5, 0.4, 0.6, 0.3, 0.7, 0.22, 0.78, 0.14, 0.86];

/**
 * Places each relationship label on its own line where it covers no other
 * label and no server card (T063): the middle first, then further out. When
 * nothing is clear, the spot with the least overlap wins.
 */
export function placeLabels(labels: { id: string; points: Point[]; width: number; height: number }[], obstacles: Rect[]): Map<string, Point> {
  const placed: Rect[] = [];
  const result = new Map<string, Point>();
  for (const label of labels) {
    let best: { at: Point; cost: number } | null = null;
    for (const fraction of FRACTIONS) {
      const at = pointAlong(label.points, fraction);
      if (!at) break;
      const box = { x: at.x - label.width / 2 - 3, y: at.y - label.height / 2 - 3, width: label.width + 6, height: label.height + 6 };
      const cost = [...placed, ...obstacles].reduce((sum, other) => sum + overlap(box, other), 0);
      if (!best || cost < best.cost) best = { at, cost };
      if (cost === 0) break;
    }
    if (!best) continue;
    result.set(label.id, best.at);
    placed.push({ x: best.at.x - label.width / 2, y: best.at.y - label.height / 2, width: label.width, height: label.height });
  }
  return result;
}

/** Rough on-screen size of a 12 px relationship label. */
export const labelSize = (text: string) => ({ width: Math.ceil(text.length * 6.6) + 16, height: 22 });
