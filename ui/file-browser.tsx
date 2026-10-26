import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { FileAction, FileEntry, FilePage, FileSession, FileResource, FileChange, FileMutationPreview, MapServer, LocalSelection } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { TransferQueue, transferActive, useTransfers } from "./transfer-queue";

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
  remote_rejected: "The server rejected the change. Check permissions, collisions or whether the folder is empty, then inspect the refreshed result.",
  outcome_unknown: "The server may have applied this change. Open a new session and inspect the target before retrying.",
};
type ChangeForm = { kind: "mkdir" | "rename" | "move" | "delete"; resourceId: string; name: string; preview: FileMutationPreview | null };
const actionLabels = { create_directory: "Create folder", rename: "Rename", move: "Move", delete: "Delete" };
export function FileBrowser({ server, onBusy }: { server: MapServer; onBusy: (value: boolean) => void }) {
  const [session, setSession] = useState<FileSession | null>(null);
  const [page, setPage] = useState<FilePage | null>(null);
  const [selected, setSelected] = useState<FileEntry | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [failure, setFailure] = useState("");
  const [pinned, setPinned] = useState<FileResource | null>(null);
  const [form, setForm] = useState<ChangeForm | null>(null);
  const [reloadTarget, setReloadTarget] = useState<FileResource | null>(null);
  const [selection, setSelection] = useState<{ local: LocalSelection; resource: FileResource } | null>(null);
  const selectedGrant = useRef<string | null>(null);
  const transfers = useTransfers();
  const jobs = transfers.rows.filter(row => row.server_id === server.id);
  const transferring = jobs.some(row => row.session_id === session?.session_id && transferActive(row));
  const transferBusy = useRef(false);
  transferBusy.current = transferring;
  const active = useRef<FileSession | null>(null);
  const operation = useRef<string | null>(null);
  const generation = useRef(0);
  const working = useRef(false);
  const mutating = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  function clear(message: string) {
    if (mutating.current) message += " The server may have applied the pending change. Reconnect and inspect before retrying.";
    mutating.current = false;
    generation.current++; working.current = false;
    clearTimeout(timer.current);
    const id = active.current?.session_id, pending = operation.current;
    active.current = null; operation.current = null;
    setSession(null); setPage(null); setSelected(null); setBusy(false); onBusy(false); setFailure(""); setNotice(message);
    setPinned(null); setForm(null); setReloadTarget(null);
    if (selectedGrant.current) void client.discardSelection(selectedGrant.current).catch(() => {});
    selectedGrant.current = null; setSelection(null);
    if (pending) void client.cancelSsh(pending).catch(() => {});
    if (id) void client.lockFiles(id).catch(() => setFailure("Session cleanup could not be confirmed. Wait for its idle expiry before closing Atlas."));
  }
  function arm(value: FileSession) {
    clearTimeout(timer.current);
    const remaining = Math.min(value.idle_seconds * 1000, Date.parse(value.absolute_expires_at) - Date.now());
    timer.current = setTimeout(() => {
      // The backend refreshes idle activity for real stream work, including
      // verification whose byte counter no longer changes. Absolute expiry
      // still wins; the backend reaper remains authoritative for every job.
      if (transferBusy.current && active.current?.session_id === value.session_id && Date.now() < Date.parse(value.absolute_expires_at)) arm(value);
      else clear("Session expired. Open a new session to continue.");
    }, Math.max(0, remaining));
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
      const id = active.current?.session_id, pending = operation.current;
      generation.current++; clearTimeout(timer.current);
      active.current = null; operation.current = null;
      // Protected cleanup must precede optional event/state teardown.
      if (id) void client.releaseFiles(id).catch(() => {});
      if (selectedGrant.current) void client.discardSelection(selectedGrant.current).catch(() => {});
      if (pending) void client.cancelSsh(pending).catch(() => {});
      disposed = true; document.removeEventListener("visibilitychange", hide);
      listeners.forEach(fn => { try { fn(); } catch {} }); onBusy(false);
    };
  }, []);
  const progressKey = jobs.filter(row => row.session_id === session?.session_id).map(row => `${row.id}:${row.bytes}:${row.state}`).join(";");
  const wasTransferring = useRef(false);
  useEffect(() => {
    const value = active.current;
    if (value && transferring) arm(value);
    if (value && wasTransferring.current && !transferring && page && !working.current) void read({ kind: "list", resource_id: page.directory.id, cursor: null });
    wasTransferring.current = transferring;
  }, [progressKey]);
  useEffect(() => {
    const preview = form?.preview;
    if (!preview) return;
    const timeout = setTimeout(() => {
      const value = active.current;
      if (value) void client.discardChange(server.id, value.session_id, preview.operation_id).catch(() => {});
      operation.current = null;
      setForm(current => current?.preview?.operation_id === preview.operation_id ? { ...current, preview: null } : current);
      setFailure("The preview expired. Review the change again before confirming.");
    }, Math.max(0, Date.parse(preview.expires_at) - Date.now()));
    return () => clearTimeout(timeout);
  }, [form?.preview?.operation_id]);

  async function fetchFiles(value: FileSession, action: FileAction, epoch: number) {
    const result = await client.readFiles(server.id, value.session_id, action);
    if (epoch !== generation.current || active.current?.session_id !== value.session_id) return;
    if (result.kind === "directory") { setPage(result.page); setSelected(null); setReloadTarget(null); }
    else if (result.kind === "metadata") setSelected(result.entry);
    else { setPinned(result.resource); setSelected(null); }
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
    if (active.current && !["denied", "not_found", "invalid_input", "conflict", "remote_rejected"].includes(code ?? "")) clear("Session closed. Open a new session to retry.");
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
  function cancelForm() {
    const preview = form?.preview, value = active.current;
    if (preview && value) void client.discardChange(server.id, value.session_id, preview.operation_id).catch(() => {});
    operation.current = null; setForm(null); setFailure("");
  }
  async function previewChange() {
    const value = active.current;
    if (!value || !form || !page || working.current) return;
    const change: FileChange = form.kind === "mkdir" ? { kind: "mkdir", parent_id: page.directory.id, name: form.name }
      : form.kind === "rename" ? { kind: "rename", resource_id: form.resourceId, name: form.name }
      : form.kind === "move" ? { kind: "move", resource_id: form.resourceId, parent_id: page.directory.id }
      : { kind: "delete", resource_id: form.resourceId };
    working.current = true; setBusy(true); setFailure(""); const epoch = generation.current;
    try {
      const preview = await client.previewChange(server.id, value.session_id, change);
      if (epoch !== generation.current) { await client.discardChange(server.id, value.session_id, preview.operation_id).catch(() => {}); return; }
      operation.current = preview.operation_id; setForm(current => current ? { ...current, preview } : null); arm(value);
    } catch (error) {
      if (epoch === generation.current && (error as { code?: string })?.code === "conflict") setFailure("The target exists or changed. Choose another name, or cancel and refresh the folder.");
      else handleFailure(error, epoch);
    } finally { if (epoch === generation.current) { working.current = false; setBusy(false); } }
  }
  async function commitChange() {
    const value = active.current, preview = form?.preview, refresh = page?.directory;
    if (!value || !preview || !refresh || working.current) return;
    working.current = true; mutating.current = true; setBusy(true); onBusy(true); setFailure("");
    const epoch = generation.current;
    setReloadTarget(refresh); setPage(null); setSelected(null);
    // Remove the approval timer once applying; expiry/lock can still revoke
    // the session. A failed acknowledgement never triggers an automatic retry.
    setForm(current => current ? { ...current, preview: null } : null);
    try {
      const receipt = await client.commitChange(server.id, value.session_id, preview.operation_id);
      if (epoch !== generation.current) return;
      mutating.current = false; operation.current = null; setForm(null); setPinned(null);
      setNotice(`${actionLabels[receipt.kind]} applied.`); setReloadTarget(receipt.refresh); arm(value);
      await fetchFiles(value, { kind: "list", resource_id: receipt.refresh.id, cursor: null }, epoch);
    } catch (error) {
      handleFailure(error, epoch);
      if (epoch === generation.current && active.current) {
        mutating.current = false; operation.current = null; setForm(null); setPinned(null);
        const message = (error as { code?: string })?.code === "conflict" ? "The target exists or changed. No replacement was requested; inspect the refreshed folder." : fileErrors[(error as { code?: string })?.code ?? ""] ?? errorMessage(error);
        try { await fetchFiles(value, { kind: "list", resource_id: refresh.id, cursor: null }, epoch); } catch (failure) { handleFailure(failure, epoch); }
        if (epoch === generation.current) setFailure(message);
      }
    } finally { if (epoch === generation.current) { working.current = false; mutating.current = false; operation.current = null; setBusy(false); onBusy(false); } }
  }
  function cancelSelection() {
    if (selectedGrant.current) void client.discardSelection(selectedGrant.current).catch(() => {});
    selectedGrant.current = null; setSelection(null);
  }
  async function pick(resource: FileResource, download: boolean) {
    const value = active.current;
    if (!value || working.current) return;
    const epoch = generation.current;
    working.current = true; setBusy(true); onBusy(true); setFailure("");
    try {
      const local = await (download ? client.pickDownload(server.id, value.session_id, resource.id) : client.pickUpload(server.id, value.session_id));
      if (!local) return;
      if (epoch !== generation.current) { await client.discardSelection(local.grant_id); return; }
      selectedGrant.current = local.grant_id; setSelection({ local, resource }); arm(value);
    } catch (error) { handleFailure(error, epoch); }
    finally { if (epoch === generation.current) { working.current = false; setBusy(false); onBusy(false); } }
  }
  async function startTransfer() {
    const value = active.current;
    if (!value || !selection || working.current) return;
    const epoch = generation.current;
    working.current = true; setBusy(true); setFailure("");
    try {
      await client.transfer(server.id, value.session_id, selection.resource.id, selection.local.grant_id);
      if (epoch === generation.current) { selectedGrant.current = null; setSelection(null); setNotice("Transfer queued. See its verified result below or in Transfers."); arm(value); }
    } catch (error) { if (epoch === generation.current) cancelSelection(); handleFailure(error, epoch); }
    finally { if (epoch === generation.current) { working.current = false; setBusy(false); } }
  }
  const browsingDisabled = busy || form !== null || selection !== null || transferring;
  return <section className="file-browser" aria-label="Remote files">
    <div className="section-heading"><div><h3>Server files</h3><p>SFTP · Existing SSH identity · Strict host verification</p></div>
      {session ? <button onClick={() => clear("Session locked. Protected file data has been cleared.")}>Lock session</button> : busy ? <button onClick={() => clear("Connection cancelled.")}>Cancel connection</button> : <button className="primary" onClick={() => void connect()}>Open file session</button>}
    </div>
    {notice && <p role="status">{notice}</p>}
    {failure && <p role="alert" className="form-error">{failure}</p>}
    {!session && !busy && <div className="unavailable"><h3>Your files stay on your server.</h3><p>Open a session to browse as your SSH user. The session locks after 5 minutes of inactivity or after 30 minutes. Leaving Files clears browsing data; queued transfers retain their session until finished.</p></div>}
    {session && <>
      <div className="file-session"><span className="state-pill">SSH authenticated · {session.profile.user}@{session.profile.hostname}</span><small>Absolute expiry {new Date(session.absolute_expires_at).toLocaleTimeString()}</small><code>{session.fingerprint}</code></div>
      {selection && <section className="file-change-review" aria-label="Review transfer"><h4>{selection.local.direction === "upload" ? "Upload selected file" : "Download selected file"}</h4><dl><dt>Server</dt><dd>{server.name} · {session.profile.user}@{session.profile.hostname}</dd><dt>Local file</dt><dd><code>{selection.local.path_label}</code></dd><dt>Remote destination / source</dt><dd><code>{selection.local.direction === "upload" ? `${selection.resource.label.replace(/\/$/, "")}/${selection.local.name}` : selection.resource.label}</code></dd></dl><p>Overwrite disabled. Content is streamed and verified before publication. Selection expires after 10 minutes.</p><button className="primary" disabled={busy} onClick={() => void startTransfer()}>Start transfer</button><button disabled={busy} onClick={cancelSelection}>Cancel selection</button></section>}
      {pinned && <div className="file-move-choice"><strong>Choose a destination folder</strong><code>{pinned.label}</code><div><button disabled={browsingDisabled || !page} onClick={() => setForm({ kind: "move", resourceId: pinned.id, name: "", preview: null })}>Move here</button><button disabled={browsingDisabled} onClick={() => void read({ kind: "clear_source", resource_id: pinned.id })}>Cancel move</button></div></div>}
      {form && <form className="file-change-review" aria-label="Review file change" onSubmit={event => { event.preventDefault(); void previewChange(); }}>
        <h4>{form.kind === "mkdir" ? "Create folder" : form.kind === "rename" ? "Rename entry" : form.kind === "move" ? "Move entry" : "Delete entry"}</h4>
        {!form.preview ? <>{["mkdir", "rename"].includes(form.kind) && <label>{form.kind === "mkdir" ? "New folder name" : "New name"}<input autoFocus required maxLength={255} aria-label={form.kind === "mkdir" ? "New folder name" : "New name"} value={form.name} disabled={busy} onChange={event => setForm(current => current ? { ...current, name: event.target.value } : null)} /></label>}<p>{form.kind === "delete" ? "Review the exact target before deleting. Folders must be empty; links are removed without deleting their targets." : "Review the source and destination before applying. Overwrite is disabled."}</p><button className="primary" type="submit" disabled={busy}>Review change</button></> : <>
          <dl><dt>Server</dt><dd>{form.preview.server} · {form.preview.endpoint}</dd>{form.preview.source && <><dt>Source ({form.preview.source_kind})</dt><dd><code>{form.preview.source}</code></dd></>}{form.preview.destination && <><dt>Destination</dt><dd><code>{form.preview.destination}</code></dd></>}</dl>
          <p>{form.kind === "delete" ? "Delete this exact entry permanently. Folders must be empty; link targets are preserved." : "Overwrite: disabled. An existing destination must be handled separately."}</p>
          <button className={form.kind === "delete" ? "danger" : "primary"} type="button" disabled={busy} onClick={() => void commitChange()}>{form.kind === "delete" ? "Confirm delete" : "Confirm change"}</button>
        </>}
        <button type="button" disabled={busy} onClick={cancelForm}>Cancel change</button>
      </form>}
      {!page && reloadTarget && !busy && <button onClick={() => openDirectory(reloadTarget.id)}>Reload folder</button>}
      {page && <><nav className="file-breadcrumbs" aria-label="Remote directory">{page.breadcrumbs.map(resource => <button disabled={browsingDisabled} key={resource.id} onClick={() => openDirectory(resource.id)}>{resource.label}</button>)}</nav>
        <div className="file-location"><code>{page.directory.label}</code><div><button disabled={busy || form !== null || selection !== null} onClick={() => void pick(page.directory, false)}>Pick upload</button><button disabled={browsingDisabled || !!pinned} onClick={() => { setFailure(""); setForm({ kind: "mkdir", resourceId: page.directory.id, name: "", preview: null }); }}>New folder</button><button disabled={browsingDisabled} onClick={() => openDirectory(page.directory.id)}>Refresh folder</button></div></div>
        <div className="file-layout"><div className="file-list"><table><thead><tr><th>Name</th><th>Type</th><th>Bytes</th></tr></thead><tbody>{page.entries.map(entry => <tr key={entry.resource.id} aria-selected={selected?.resource.id === entry.resource.id}><td><button className="text-button" disabled={browsingDisabled} onClick={() => void read({ kind: "stat", resource_id: entry.resource.id })}>{entry.resource.label}</button>{entry.kind === "directory" && <button className="file-open" disabled={browsingDisabled} aria-label={`Open folder ${entry.resource.label}`} onClick={() => openDirectory(entry.resource.id)}>Open →</button>}</td><td>{entry.kind}</td><td>{entry.size_bytes ?? "Unknown"}</td></tr>)}</tbody></table>{!page.entries.length && <p className="empty-copy">This page contains no entries.</p>}</div>
          <aside className="file-details" aria-label="Selected file">{selected ? <><h4>{selected.resource.label}</h4><dl><dt>Type</dt><dd>{selected.kind}</dd><dt>Size (bytes)</dt><dd>{selected.size_bytes ?? "Unknown"}</dd><dt>Permissions</dt><dd>{selected.permissions ?? "Unknown"}</dd><dt>Owner / group ID</dt><dd>{selected.owner ?? "?"} / {selected.group ?? "?"}</dd><dt>Modified</dt><dd>{selected.modified_at ? new Date(selected.modified_at).toLocaleString() : "Unknown"}</dd></dl>{selected.kind === "file" && <button disabled={busy || form !== null || selection !== null} onClick={() => void pick(selected.resource, true)}>Download selected file</button>}{selected.kind === "symlink" && <button disabled={browsingDisabled} onClick={() => void read({ kind: "follow_link", resource_id: selected.resource.id })}>Follow link to folder</button>}{["file", "directory", "symlink"].includes(selected.kind) && <div className="file-entry-actions"><button disabled={browsingDisabled || !!pinned} aria-label="Rename selected entry" onClick={() => setForm({ kind: "rename", resourceId: selected.resource.id, name: "", preview: null })}>Rename</button><button disabled={browsingDisabled || !!pinned} aria-label="Move selected entry" onClick={() => void read({ kind: "pin_source", resource_id: selected.resource.id })}>Move</button><button disabled={browsingDisabled || !!pinned} aria-label="Delete selected entry" onClick={() => setForm({ kind: "delete", resourceId: selected.resource.id, name: "", preview: null })}>Delete</button></div>}</> : <p>Select a name to inspect its metadata.</p>}</aside></div>
        <div className="file-page"><span>{busy ? "Reading server…" : `${page.entries.length} entries on this page`}</span><button disabled={browsingDisabled || !page.next_cursor} onClick={() => void read({ kind: "list", resource_id: page.directory.id, cursor: page.next_cursor })}>Next page</button></div>
      </>}
      <p className="muted">Uses your SSH account’s permissions. Changes require confirmation. Transfers publish only verified files and never overwrite. Recursive deletion and sudo are unavailable.</p>
    </>}
    {jobs.length > 0 && <TransferQueue rows={jobs} failure={transfers.failure} />}
  </section>;
}
