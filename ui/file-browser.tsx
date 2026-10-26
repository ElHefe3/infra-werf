import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { FileAction, FileEntry, FilePage, FileSession, MapServer } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";

const client = createWorkspaceClient();
const fileErrors: Record<string, string> = {
  host_trust_failure: "The SSH host identity needs review. Verify it independently through your existing SSH client before retrying.",
  unauthenticated: "SSH authentication failed. Check your configured identity or unlock your existing SSH agent.",
  io_failure: "The SSH/SFTP connection failed or disconnected. Check the server and LAN/VPN connection.",
  denied: "Your SSH account cannot access this target, or its handle is no longer valid.",
  not_found: "The remote target no longer exists.",
  expired_session: "The file session has expired. Open a new session to continue.",
  output_limit: "The server response exceeded the safety limit. The session was closed without keeping partial data.",
  timed_out: "The file operation timed out. The session was closed.",
  conflict: "Another file operation is still running. Wait for it to finish.",
};
export function FileBrowser({ server, onBusy }: { server: MapServer; onBusy: (value: boolean) => void }) {
  const [session, setSession] = useState<FileSession | null>(null);
  const [page, setPage] = useState<FilePage | null>(null);
  const [selected, setSelected] = useState<FileEntry | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [failure, setFailure] = useState("");
  const active = useRef<FileSession | null>(null);
  const operation = useRef<string | null>(null);
  const generation = useRef(0);
  const working = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  function clear(message: string) {
    generation.current++; working.current = false;
    clearTimeout(timer.current);
    const id = active.current?.session_id, pending = operation.current;
    active.current = null; operation.current = null;
    setSession(null); setPage(null); setSelected(null); setBusy(false); onBusy(false); setFailure(""); setNotice(message);
    if (pending) void client.cancelSsh(pending).catch(() => {});
    if (id) void client.lockFiles(id).catch(() => setFailure("Session cleanup could not be confirmed. Wait for its idle expiry before closing Atlas."));
  }
  function arm(value: FileSession) {
    clearTimeout(timer.current);
    const remaining = Math.min(value.idle_seconds * 1000, Date.parse(value.absolute_expires_at) - Date.now());
    timer.current = setTimeout(() => clear("Session expired. Open a new session to continue."), Math.max(0, remaining));
  }
  useEffect(() => {
    let disposed = false; const listeners: (() => void)[] = [];
    const keep = (fn: () => void) => { if (disposed) fn(); else listeners.push(fn); };
    void listen<string | null>("atlas-session-closed", event => {
      if (active.current && (event.payload === null || event.payload === active.current.session_id)) clear("Session closed or expired. Protected file data has been cleared.");
    }).then(keep).catch(() => {});
    void listen<string>("atlas-close-blocked", event => setFailure(event.payload)).then(keep).catch(() => {});
    const hide = () => { if (document.visibilityState === "hidden") clear("Session locked while Atlas was hidden."); };
    document.addEventListener("visibilitychange", hide);
    return () => {
      disposed = true; listeners.forEach(fn => fn()); document.removeEventListener("visibilitychange", hide);
      generation.current++; clearTimeout(timer.current); onBusy(false);
      if (operation.current) void client.cancelSsh(operation.current).catch(() => {});
      if (active.current) void client.lockFiles(active.current.session_id).catch(() => {});
      active.current = null; operation.current = null;
    };
  }, []);

  async function fetchFiles(value: FileSession, action: FileAction, epoch: number) {
    const result = await client.readFiles(server.id, value.session_id, action);
    if (epoch !== generation.current || active.current?.session_id !== value.session_id) return;
    if (result.kind === "directory") { setPage(result.page); setSelected(null); }
    else setSelected(result.entry);
    arm(value);
  }
  async function connect() {
    if (working.current || active.current) return;
    working.current = true; const epoch = ++generation.current;
    setBusy(true); onBusy(true); setFailure(""); setNotice("");
    try {
      const ticket = await client.prepareFiles(server.id);
      if (epoch !== generation.current) { await client.cancelSsh(ticket.operation_id); return; }
      operation.current = ticket.operation_id;
      const value = await client.openFiles(server.id, ticket.operation_id);
      operation.current = null;
      if (epoch !== generation.current) { await client.lockFiles(value.session_id); return; }
      active.current = value; setSession(value); arm(value);
      await fetchFiles(value, { kind: "list", resource_id: value.home.id, cursor: null }, epoch);
    } catch (error) { handleFailure(error, epoch); }
    finally { if (epoch === generation.current) { working.current = false; operation.current = null; setBusy(false); onBusy(false); } }
  }
  function handleFailure(error: unknown, epoch: number) {
    if (epoch !== generation.current) return;
    const code = (error as { code?: string })?.code;
    if (active.current && !["denied", "not_found", "invalid_input", "conflict"].includes(code ?? "")) clear("Session closed. Open a new session to retry.");
    setFailure(fileErrors[code ?? ""] ?? errorMessage(error));
  }
  async function read(action: FileAction) {
    const value = active.current;
    if (!value || working.current) return;
    working.current = true; setBusy(true); setFailure("");
    const epoch = generation.current;
    try { await fetchFiles(value, action, epoch); }
    catch (error) { handleFailure(error, epoch); }
    finally { if (epoch === generation.current) { working.current = false; setBusy(false); } }
  }
  const openDirectory = (id: string) => void read({ kind: "list", resource_id: id, cursor: null });
  return <section className="file-browser" aria-label="Remote files">
    <div className="section-heading"><div><h3>Server files</h3><p>Read-only SFTP · Existing SSH identity · Strict host verification</p></div>
      {session ? <button onClick={() => clear("Session locked. Protected file data has been cleared.")}>Lock session</button> : busy ? <button onClick={() => clear("Connection cancelled.")}>Cancel connection</button> : <button className="primary" onClick={() => void connect()}>Open file session</button>}
    </div>
    {notice && <p role="status">{notice}</p>}
    {failure && <p role="alert" className="form-error">{failure}</p>}
    {!session && !busy && <div className="unavailable"><h3>Your files stay on your server.</h3><p>Open a session to browse as your SSH user. The session locks after 5 minutes of inactivity, after 30 minutes, or when you leave Files.</p></div>}
    {session && <>
      <div className="file-session"><span className="state-pill">SSH authenticated · {session.profile.user}@{session.profile.hostname}</span><small>Absolute expiry {new Date(session.absolute_expires_at).toLocaleTimeString()}</small><code>{session.fingerprint}</code></div>
      {page && <><nav className="file-breadcrumbs" aria-label="Remote directory">{page.breadcrumbs.map(resource => <button disabled={busy} key={resource.id} onClick={() => openDirectory(resource.id)}>{resource.label}</button>)}</nav>
        <div className="file-location"><code>{page.directory.label}</code><button disabled={busy} onClick={() => openDirectory(page.directory.id)}>Refresh folder</button></div>
        <div className="file-layout"><div className="file-list"><table><thead><tr><th>Name</th><th>Type</th><th>Bytes</th></tr></thead><tbody>{page.entries.map(entry => <tr key={entry.resource.id} aria-selected={selected?.resource.id === entry.resource.id}><td><button className="text-button" disabled={busy} onClick={() => void read({ kind: "stat", resource_id: entry.resource.id })}>{entry.resource.label}</button>{entry.kind === "directory" && <button className="file-open" disabled={busy} aria-label={`Open folder ${entry.resource.label}`} onClick={() => openDirectory(entry.resource.id)}>Open →</button>}</td><td>{entry.kind}</td><td>{entry.size_bytes ?? "Unknown"}</td></tr>)}</tbody></table>{!page.entries.length && <p className="empty-copy">This page contains no entries.</p>}</div>
          <aside className="file-details" aria-label="Selected file">{selected ? <><h4>{selected.resource.label}</h4><dl><dt>Type</dt><dd>{selected.kind}</dd><dt>Size (bytes)</dt><dd>{selected.size_bytes ?? "Unknown"}</dd><dt>Permissions</dt><dd>{selected.permissions ?? "Unknown"}</dd><dt>Owner / group ID</dt><dd>{selected.owner ?? "?"} / {selected.group ?? "?"}</dd><dt>Modified</dt><dd>{selected.modified_at ? new Date(selected.modified_at).toLocaleString() : "Unknown"}</dd></dl>{selected.kind === "symlink" && <button disabled={busy} onClick={() => void read({ kind: "follow_link", resource_id: selected.resource.id })}>Follow link to folder</button>}</> : <p>Select a name to inspect its metadata.</p>}</aside></div>
        <div className="file-page"><span>{busy ? "Reading server…" : `${page.entries.length} entries on this page`}</span><button disabled={busy || !page.next_cursor} onClick={() => void read({ kind: "list", resource_id: page.directory.id, cursor: page.next_cursor })}>Next page</button></div>
      </>}
      <p className="muted">Browsing uses your SSH account’s permissions. Changes, uploads, downloads and sudo are unavailable in this build.</p>
    </>}
  </section>;
}
