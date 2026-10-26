import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { TransferView } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";

const client = createWorkspaceClient();
const terminal = new Set(["complete", "cancelled", "failed", "outcome_unknown"]);
export const transferActive = (row: TransferView) => !terminal.has(row.state);
export function useTransfers() {
  const [rows, setRows] = useState<TransferView[]>([]);
  const [failure, setFailure] = useState("");
  useEffect(() => {
    let disposed = false, revision = 0;
    let timer: ReturnType<typeof setTimeout> | undefined, unlisten: (() => void) | undefined;
    const refresh = async () => {
      const epoch = revision;
      try { const values = await client.transfers(); if (!disposed && epoch === revision) { setRows(values); setFailure(""); } }
      catch (error) { if (!disposed) { setRows([]); setFailure(errorMessage(error)); } }
      if (!disposed) timer = setTimeout(() => void refresh(), 400);
    };
    void listen<string | null>("atlas-session-closed", event => {
      revision++;
      setRows(current => current.map(row => event.payload === null || row.session_id === event.payload ? { ...row, remote: null, local_name: null, checksum: null } : row));
    }).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {});
    void refresh();
    return () => { disposed = true; clearTimeout(timer); unlisten?.(); };
  }, []);
  return { rows, failure };
}
export function TransferQueue({ rows, failure = "", retry }: { rows: TransferView[]; failure?: string; retry?: (serverId: string) => void }) {
  const [error, setError] = useState("");
  async function cancel(id: string) { try { await client.cancelTransfer(id); setError(""); } catch (failure) { setError(errorMessage(failure)); } }
  return <section className="transfer-queue" aria-label="Transfer queue">
    <div className="section-heading"><div><h3>Transfers</h3><p>Queued work continues when you leave Files. Lock stops its session. Retry starts a new reviewed attempt.</p></div></div>
    {(failure || error) && <p role="alert" className="form-error">{failure || error}</p>}
    {!rows.length && <p className="empty-copy">No transfers in this InfraWerf session. Open a server’s Files tab to upload or download.</p>}
    {rows.map(row => <article className="transfer-row" key={row.id} aria-label={`${row.direction} ${row.local_name ?? "protected file"}`}>
      <div><strong>{row.direction === "upload" ? "Upload" : "Download"} · {row.local_name ?? "Protected file details cleared"}</strong><span className="state-pill">{row.state.replaceAll("_", " ")}</span></div>
      {row.remote && <code>{row.remote}</code>}
      <progress aria-label="Transferred bytes" value={Number(row.bytes)} max={Math.max(1, Number(row.total ?? row.bytes))} />
      <small>{Number(row.bytes).toLocaleString()} / {row.total === null ? "unknown" : Number(row.total).toLocaleString()} bytes{row.verified ? " · SHA-256 verified" : ""}</small>
      {row.checksum && <code className="transfer-checksum">SHA-256 {row.checksum}</code>}
      {row.failure && <p role="status">{row.state === "outcome_unknown" ? "Publication could not be confirmed. Reconnect and inspect the destination before another attempt." : row.state === "cancelled" ? "Transfer stopped. A retry requires a new selection." : `Transfer stopped (${row.failure.replaceAll("_", " ")}). Inspect the destination before retrying.`}</p>}
      {row.cleanup_needed && <p>Restricted upload staging may remain on the server. Reconnect and inspect before removing it.</p>}
      <div>{transferActive(row) ? <button disabled={row.state === "cancelling" || row.state === "publishing"} onClick={() => void cancel(row.id)}>{row.state === "cancelling" ? "Stopping…" : row.state === "publishing" ? "Publishing…" : "Cancel transfer"}</button> : retry && <button onClick={() => retry(row.server_id)}>Open Files for a new attempt</button>}</div>
    </article>)}
  </section>;
}
