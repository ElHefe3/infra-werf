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
