import React from "react";
import type { MapState } from "./contracts.gen";
import { inventoryRows } from "./map-model";

// Small presentation pieces shared by the application shell and the built-in
// extension renderers.
export const date = (value?: string | null) => value ? new Date(value).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }) : "No observation";
export const memory = (kib: number) => `${(kib / 1024 / 1024).toLocaleString([], { maximumFractionDigits: 1 })} GiB`;

export function Notice({ children }: { children: React.ReactNode }) { return <div className="notice"><span aria-hidden>◇</span><div>{children}</div></div>; }

export function SavedInventory({ state, query, open, single = false }: { state: MapState; query: string; open: (id: string) => void; single?: boolean }) {
  const rows = inventoryRows(state, query);
  return <div className="inventory-content"><div className="inventory-summary"><strong>{rows.length.toLocaleString()} saved workloads</strong><span>Reported state · not a live check</span></div>
    <div className="table-scroll"><table><thead><tr>{!single && <th>Server</th>}<th>Workload</th><th>Scope / engine</th><th>Reported state</th></tr></thead><tbody>
      {rows.map(({ server, workload }) => <tr key={workload.id}>{!single && <td><button className="text-button" onClick={() => open(server.id)}>{server.name}</button></td>}<td><strong>{workload.name}</strong><small>{workload.kind === "container" ? workload.detail : workload.kind}</small></td><td>{workload.scope}<small>{workload.engine}</small></td><td><span className="state-pill">{workload.state || "Unknown"}</span>{workload.kind === "service" && <small>{workload.detail}</small>}</td></tr>)}
    </tbody></table>{!rows.length && <p className="empty-copy">{query ? "No saved workloads match your search." : "No saved workloads have been imported."}</p>}</div>
  </div>;
}
