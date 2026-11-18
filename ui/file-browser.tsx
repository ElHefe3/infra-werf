import { useEffect, useRef, useState } from "react";
import { Secret } from "./pin";
import { listen } from "@tauri-apps/api/event";
import type { FileAction, FileEntry, FilePage, FileSession, FileResource, FileChange, FileMutationPreview, MapServer, LocalSelection, ProtectedPlacementStage, ProtectedPlacementReceipt, ElevationPreview, ProtectedPlacementRequest } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { fileErrors, lateFailure } from "./file-session-model";
import { TransferQueue, transferActive, useTransfers } from "./transfer-queue";

const client = createWorkspaceClient();
type ChangeForm = { kind: "mkdir" | "rename" | "move" | "delete"; resourceId: string; name: string; preview: FileMutationPreview | null };
type PlacementForm = { resource: FileResource; local: LocalSelection | null; stage: ProtectedPlacementStage | null; destination: string; owner: string; group: string; mode: string; preview: ElevationPreview | null; receipt: ProtectedPlacementReceipt | null };
type PendingPlacementCleanup = { serverId: string; sessionId: string; resourceId: string; location: string };
const pendingPlacementCleanups = new Map<string, PendingPlacementCleanup>();
const expiredPlacementWarnings = new Map<string, string>();
const actionLabels = { create_directory: "Create folder", rename: "Rename", move: "Move", delete: "Delete" };
// The backend closes a session when its server's host is edited, and the map
// refresh that follows remounts this view for the new host. The explanation
// must survive that remount, so it is carried briefly per server.
const carried = new Map<string, { message: string; at: number }>();
/** Reads without consuming: React may run a state initialiser more than once. Entries simply expire. */
function takeCarried(serverId: string): string {
  const entry = carried.get(serverId);
  return entry && Date.now() - entry.at < 15_000 ? entry.message : "";
}

export function FileBrowser({ server, onBusy }: { server: MapServer; onBusy: (value: boolean) => void }) {
  const [session, setSession] = useState<FileSession | null>(null);
  const [page, setPage] = useState<FilePage | null>(null);
  const [selected, setSelected] = useState<FileEntry | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState(() => takeCarried(server.id));
  // When a host edit remounts this view, React renders the new instance before the old one's
  // cleanup has recorded the notice, so look once more after mounting.
  useEffect(() => { const carriedNotice = takeCarried(server.id); if (carriedNotice) setNotice(current => current || carriedNotice); }, [server.id]);
  const [failure, setFailure] = useState("");
  const [pinned, setPinned] = useState<FileResource | null>(null);
  const [form, setForm] = useState<ChangeForm | null>(null);
  const [reloadTarget, setReloadTarget] = useState<FileResource | null>(null);
  const [selection, setSelection] = useState<{ local: LocalSelection; resource: FileResource } | null>(null);
  const [placement, setPlacement] = useState<PlacementForm | null>(null);
  const [pendingCleanup, setPendingCleanup] = useState<PendingPlacementCleanup | null>(() => pendingPlacementCleanups.get(server.id) ?? null);
  const [expiredPlacement, setExpiredPlacement] = useState<string | null>(() => expiredPlacementWarnings.get(server.id) ?? null);
  const placementRef = useRef<PlacementForm | null>(null);
  placementRef.current = placement;
  const placementPassword = useRef<HTMLInputElement>(null);
  const stagingTask = useRef<Promise<ProtectedPlacementStage> | null>(null);
  const placementResource = useRef<FileResource | null>(null);
  const selectedGrant = useRef<string | null>(null);
  const transfers = useTransfers();
  const jobs = transfers.rows.filter(row => row.server_id === server.id);
  const transferring = jobs.some(row => row.session_id === session?.session_id && transferActive(row));
  const transferBusy = useRef(false);
  transferBusy.current = transferring;
  const active = useRef<FileSession | null>(null);
  const operation = useRef<string | null>(null);
  const generation = useRef(0);
  /** The generation a closing notice left while a request was in flight; see lateFailure. */
  const closedAt = useRef<number | null>(null);
  const working = useRef(false);
  const staging = useRef(false);
  const mutating = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  function updatePlacement(update: (current: PlacementForm | null) => PlacementForm | null) {
    const next = update(placementRef.current);
    placementRef.current = next;
    setPlacement(next);
  }

  function clear(message: string, carry = false) {
    if (mutating.current) message += " The server may have applied the pending change. Reconnect and inspect before retrying.";
    if (stagingTask.current) message = "Stopping private staging and cleaning up before the session is locked.";
    mutating.current = false;
    generation.current++; working.current = false;
    staging.current = false;
    clearTimeout(timer.current);
    const id = active.current?.session_id, pending = operation.current, staged = placementRef.current, pendingStage = stagingTask.current;
    const cleanupResourceId = staged?.resource.id ?? placementResource.current?.id;
    active.current = null; operation.current = null;
    setSession(null); setPage(null); setSelected(null); setBusy(false); onBusy(false); setFailure(""); setNotice(message);
    if (carry) carried.set(server.id, { message, at: Date.now() });
    setPinned(null); setForm(null); setReloadTarget(null); updatePlacement(() => null); placementResource.current = null;
    if (placementPassword.current) placementPassword.current.value = "";
    if (selectedGrant.current) void client.discardSelection(selectedGrant.current).catch(() => {});
    selectedGrant.current = null; setSelection(null);
    if (pending) void client.cancelSsh(pending).catch(() => {});
    if (id) void (async () => {
      let completedStage: ProtectedPlacementStage | null = null;
      let stageFailure: unknown = null;
      let cleanupUnconfirmed = false;
      if (pendingStage) {
        await client.lockFiles(id).catch(() => {});
        try { completedStage = await pendingStage; } catch (error) { stageFailure = error; }
      }
      const retained = staged?.stage ?? (!staged?.receipt ? completedStage : null);
      const shouldCleanup = (retained && (retained.cleanup_needed || !staged?.receipt || staged.receipt.cleanup_needed))
        || (!!pendingStage && (!!completedStage || (!!stageFailure && (stageFailure as { code?: string })?.code !== "cancelled")));
      if (shouldCleanup) {
        try {
          const resourceId = cleanupResourceId;
          if (!resourceId) throw new Error("Staging directory reference unavailable");
          await client.cancelProtectedPlacement(server.id, id, resourceId);
        }
        catch (error) {
          if ((error as { code?: string })?.code === "not_found") { /* The stage was already removed. */ }
          else {
            cleanupUnconfirmed = true;
            if (cleanupResourceId) {
              const recovery = { serverId: server.id, sessionId: id, resourceId: cleanupResourceId, location: retained?.staging_location ?? "private staging path unavailable" };
              pendingPlacementCleanups.set(server.id, recovery); setPendingCleanup(recovery);
              setFailure(`Could not remove the private staged file. It remains restricted at ${recovery.location}. Retry cleanup before opening another file session.`);
            } else setFailure("Could not verify cleanup of the in-flight private stage. Its restricted staging location could not be recovered.");
          }
        }
      }
      if (!cleanupUnconfirmed) {
        try { await client.lockFiles(id); }
        catch { setFailure("Session cleanup could not be confirmed. Wait for its idle expiry before closing InfraWerf."); }
      }
      if (pendingStage && !cleanupUnconfirmed) setNotice("Session locked. In-flight private staging was stopped and removed.");
    })();
  }
  function arm(value: FileSession) {
    clearTimeout(timer.current);
    const remaining = Math.min(value.idle_seconds * 1000, Date.parse(value.absolute_expires_at) - Date.now());
    timer.current = setTimeout(() => {
      // The backend refreshes idle activity for real stream work, including
      // verification whose byte counter no longer changes. Absolute expiry
      // still wins; the backend reaper remains authoritative for every job.
      if ((transferBusy.current || staging.current) && active.current?.session_id === value.session_id && Date.now() < Date.parse(value.absolute_expires_at)) arm(value);
      else clear("Session expired. Open a new session to continue.");
    }, Math.max(0, remaining));
  }
  useEffect(() => {
    let disposed = false; const listeners: (() => void)[] = [];
    const keep = (fn: () => void) => { if (disposed) fn(); else listeners.push(fn); };
    void listen<string | null>("atlas-session-closed", event => {
      if (active.current && (event.payload === null || event.payload === active.current.session_id)) {
        const inFlight = working.current;
        clear("Session closed or expired. Protected file data has been cleared.", true);
        closedAt.current = inFlight ? generation.current : null;
      }
    }).then(keep).catch(() => {});
    void listen<string>("atlas-close-blocked", event => setFailure(event.payload)).then(keep).catch(() => {});
    const hide = () => { if (document.visibilityState === "hidden") clear("Session locked while InfraWerf was hidden."); };
    document.addEventListener("visibilitychange", hide);
    return () => {
      const id = active.current?.session_id, pending = operation.current, staged = placementRef.current, pendingStage = stagingTask.current;
      const cleanupResourceId = staged?.resource.id ?? placementResource.current?.id;
      generation.current++; clearTimeout(timer.current);
      // An open session ends with this view. If the view comes straight back (a host edit remounts it), say so there.
      if (id && !(Date.now() - (carried.get(server.id)?.at ?? 0) < 15_000)) carried.set(server.id, { message: "Session closed or expired. Protected file data has been cleared.", at: Date.now() });
      active.current = null; operation.current = null;
      // Protected cleanup must precede optional event/state teardown.
      if (id) void (async () => {
        let completedStage: ProtectedPlacementStage | null = null;
        if (pendingStage) {
          await client.lockFiles(id).catch(() => {});
          try { completedStage = await pendingStage; } catch {}
        }
        const retained = staged?.stage ?? (!staged?.receipt ? completedStage : null);
        const shouldCleanup = (retained && (retained.cleanup_needed || !staged?.receipt || staged.receipt.cleanup_needed))
          || (!!pendingStage && !staged?.receipt);
        if (shouldCleanup) {
          if (cleanupResourceId) {
            try { await client.cancelProtectedPlacement(server.id, id, cleanupResourceId); }
            catch (error) {
              if ((error as { code?: string })?.code !== "not_found") pendingPlacementCleanups.set(server.id, {
                serverId: server.id, sessionId: id, resourceId: cleanupResourceId,
                location: retained?.staging_location ?? "private staging path unavailable",
              });
            }
          }
        }
        await client.releaseFiles(id).catch(() => {});
      })();
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
  useEffect(() => {
    const preview = placement?.preview;
    if (!preview) return;
    const timeout = setTimeout(() => {
      const value = active.current;
      if (value) void client.cancelElevation(server.id, value.session_id, preview.operation_id).catch(() => {});
      operation.current = null;
      if (placementPassword.current) placementPassword.current.value = "";
      updatePlacement(current => current?.preview?.operation_id === preview.operation_id ? { ...current, preview: null } : current);
      setFailure("The protected placement review expired. Review the exact destination and permissions again.");
    }, Math.max(0, Date.parse(preview.expires_at) - Date.now()));
    return () => clearTimeout(timeout);
  }, [placement?.preview?.operation_id]);

  async function fetchFiles(value: FileSession, action: FileAction, epoch: number) {
    const result = await client.readFiles(server.id, value.session_id, action);
    if (epoch !== generation.current || active.current?.session_id !== value.session_id) return;
    if (result.kind === "directory") { setPage(result.page); setSelected(null); setReloadTarget(null); }
    else if (result.kind === "metadata") setSelected(result.entry);
    else { setPinned(result.resource); setSelected(null); }
    arm(value);
  }
  async function connect() {
    if (working.current || active.current || pendingCleanup) return;
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
  async function retryPendingCleanup() {
    const pending = pendingCleanup;
    if (!pending || busy) return;
    setBusy(true); setFailure(""); setNotice("");
    try {
      await client.cancelProtectedPlacement(pending.serverId, pending.sessionId, pending.resourceId);
      await client.lockFiles(pending.sessionId).catch(() => {});
      pendingPlacementCleanups.delete(server.id); setPendingCleanup(null);
      setNotice("The restricted staged file was removed. You can open a new file session.");
    } catch (error) {
      const code = (error as { code?: string })?.code;
      if (code === "not_found") {
        pendingPlacementCleanups.delete(server.id); setPendingCleanup(null);
        setNotice("The retained stage handle is gone, so there is no private stage left to remove.");
      } else if (code === "expired_session") {
        pendingPlacementCleanups.delete(server.id); setPendingCleanup(null);
        expiredPlacementWarnings.set(server.id, pending.location); setExpiredPlacement(pending.location);
        setFailure("The previous file session expired before cleanup could be confirmed. The staged directory remains private; open a new session to inspect and remove it with your SSH account.");
      } else setFailure(`Private stage cleanup could not be confirmed: ${errorMessage(error)}. Restricted file: ${pending.location}`);
    } finally { setBusy(false); }
  }
  function handleFailure(error: unknown, epoch: number) {
    const code = (error as { code?: string })?.code;
    if (epoch !== generation.current) {
      const reason = lateFailure(closedAt.current, epoch, generation.current, code);
      if (reason) { closedAt.current = null; setFailure(reason); }
      return;
    }
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
  async function pickProtected(resource: FileResource) {
    const value = active.current;
    if (!value || working.current || placement) return;
    const epoch = generation.current;
    working.current = true; setBusy(true); onBusy(true); setFailure("");
    try {
      const local = await client.pickUpload(server.id, value.session_id);
      if (!local) return;
      if (epoch !== generation.current) { await client.discardSelection(local.grant_id); return; }
      selectedGrant.current = local.grant_id;
      placementResource.current = resource;
      updatePlacement(() => ({ resource, local, stage: null, destination: "", owner: "", group: "", mode: "0644", preview: null, receipt: null }));
      arm(value);
    } catch (error) { handleFailure(error, epoch); }
    finally { if (epoch === generation.current) { working.current = false; setBusy(false); onBusy(false); } }
  }
  async function stageProtected() {
    const value = active.current, current = placement;
    if (!value || !current?.local || working.current) return;
    const epoch = generation.current;
    staging.current = true;
    working.current = true; setBusy(true); onBusy(true); setFailure("");
    const task = client.stageProtectedPlacement(server.id, value.session_id, current.resource.id, current.local.grant_id);
    stagingTask.current = task;
    try {
      const stage = await task;
      if (epoch !== generation.current) return;
      selectedGrant.current = null;
      updatePlacement(form => form ? { ...form, local: null, stage } : null);
      if (stage.cleanup_needed) setFailure(`The upload could not be fully verified. Remove the restricted staging file at ${stage.staging_location} before continuing.`);
      else setNotice("Upload staged privately and verified. Review the destination, numeric owner/group IDs, and mode before applying with sudo.");
      arm(value);
    } catch (error) { handleFailure(error, epoch); }
    finally {
      if (stagingTask.current === task) stagingTask.current = null;
      staging.current = false;
      if (epoch === generation.current) { working.current = false; setBusy(false); onBusy(false); }
    }
  }
  function cancelProtectedSelection() {
    const current = placement;
    if (current?.local) {
      if (selectedGrant.current) void client.discardSelection(selectedGrant.current).catch(() => {});
      selectedGrant.current = null;
      updatePlacement(() => null); placementResource.current = null;
    }
    if (placementPassword.current) placementPassword.current.value = "";
    setFailure("");
  }
  async function reviewProtectedPlacement() {
    const value = active.current, current = placement;
    if (!value || !current?.stage || current.stage.cleanup_needed || !current.stage.sha256 || working.current) return;
    const modeText = current.mode.trim();
    if (!current.destination.startsWith("/") || !/^[0-9]+$/.test(current.owner) || !/^[0-9]+$/.test(current.group) || !/^[0-7]{3,4}$/.test(modeText)) {
      setFailure("Enter an absolute destination, numeric owner and group IDs, and a three or four digit octal mode.");
      return;
    }
    const owner = Number(current.owner), group = Number(current.group), mode = Number.parseInt(modeText, 8);
    if (!Number.isSafeInteger(owner) || owner > 4_294_967_295 || !Number.isSafeInteger(group) || group > 4_294_967_295 || !Number.isInteger(mode) || mode > 0o777) {
      setFailure("Owner and group IDs must fit a 32-bit unsigned integer, and mode must not include special bits.");
      return;
    }
    const request: ProtectedPlacementRequest = { destination: current.destination, owner, group, mode };
    working.current = true; setBusy(true); setFailure(""); const epoch = generation.current;
    try {
      const preview = await client.previewProtectedPlacement(server.id, value.session_id, request);
      if (epoch !== generation.current) { await client.cancelElevation(server.id, value.session_id, preview.operation_id).catch(() => {}); return; }
      operation.current = preview.operation_id;
      updatePlacement(form => form ? { ...form, preview } : null);
      arm(value);
    } catch (error) { handleFailure(error, epoch); }
    finally { if (epoch === generation.current) { working.current = false; setBusy(false); } }
  }
  async function confirmProtectedPlacement(event: React.FormEvent) {
    event.preventDefault();
    const value = active.current, current = placement, preview = current?.preview, secret = placementPassword.current?.value;
    if (!value || !current || !preview || !secret || working.current) return;
    working.current = true; setBusy(true); onBusy(true); setFailure(""); setNotice(""); const epoch = generation.current;
    const request = client.confirmProtectedPlacement(server.id, value.session_id, preview.operation_id, secret);
    if (placementPassword.current) placementPassword.current.value = "";
    try {
      const receipt = await request;
      if (epoch !== generation.current) return;
      operation.current = null;
      updatePlacement(form => form ? { ...form, preview: null, receipt } : null);
      setNotice(receipt.cleanup_needed
        ? `Placement succeeded. Remove the remaining private staged file at ${receipt.staging_location ?? current.stage?.staging_location ?? "the reported staging path"}.`
        : "Protected placement succeeded. The staged source was removed after destination verification.");
      arm(value);
    } catch (error) {
      if (epoch !== generation.current) return;
      operation.current = null;
      const code = (error as { code?: string })?.code;
      if (["expired_session", "io_failure", "host_trust_failure", "timed_out", "output_limit", "outcome_unknown"].includes(code ?? "")) {
        clear("The protected placement outcome needs inspection. The staged file may remain private on the server.");
        return;
      }
      updatePlacement(form => form ? { ...form, preview: null } : null);
      setFailure(code === "unauthenticated"
        ? "The fresh sudo password was rejected. The staged file is unchanged; review again to retry with a new password."
        : code === "conflict"
          ? "The destination already exists or changed. Nothing was overwritten; choose another path and review again."
          : code === "denied"
            ? "The server rejected the destination or permission request. Check the parent directory and requested owner, group, and mode."
            : errorMessage(error));
    } finally { if (epoch === generation.current) { working.current = false; setBusy(false); onBusy(false); } }
  }
  async function cancelProtectedReview() {
    if (placementPassword.current) placementPassword.current.value = "";
    const value = active.current, preview = placement?.preview;
    if (value && preview) try { await client.cancelElevation(server.id, value.session_id, preview.operation_id); }
    catch (error) { setFailure(errorMessage(error)); return; }
    operation.current = null;
    updatePlacement(current => current ? { ...current, preview: null } : null);
    setFailure("");
  }
  async function removeProtectedStage() {
    const value = active.current, current = placement;
    if (!value || !current?.stage || current.preview || working.current) return;
    working.current = true; setBusy(true); setFailure("");
    try {
      await client.cancelProtectedPlacement(server.id, value.session_id, current.resource.id);
      if (current.receipt) {
        updatePlacement(form => form ? {
          ...form,
          stage: form.stage ? { ...form.stage, cleanup_needed: false } : null,
          receipt: form.receipt ? { ...form.receipt, cleanup_needed: false, staging_location: null } : null,
        } : null);
        setNotice("The private staged copy was removed.");
      } else {
        updatePlacement(() => null); placementResource.current = null;
        setNotice("The private staged copy was removed. The destination was not changed.");
      }
      arm(value);
    } catch (error) { setFailure(`Private stage cleanup could not be confirmed: ${errorMessage(error)}. Restricted file: ${current.stage.staging_location}`); }
    finally { working.current = false; setBusy(false); }
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
  const browsingDisabled = busy || form !== null || selection !== null || placement !== null || transferring;
  return <section className="file-browser" aria-label="Remote files">
    <div className="section-heading"><div><h3>Server files</h3><p>SFTP · Existing SSH identity · Strict host verification</p></div>
      {session ? <button onClick={() => clear("Session locked. Protected file data has been cleared.")}>Lock session</button> : busy ? <button onClick={() => clear("Connection cancelled.")}>Cancel connection</button> : pendingCleanup ? <button className="primary" onClick={() => void retryPendingCleanup()}>Retry private cleanup</button> : <button className="primary" onClick={() => void connect()}>Open file session</button>}
    </div>
    {notice && <p role="status">{notice}</p>}
    {failure && <p role="alert" className="form-error">{failure}</p>}
    {expiredPlacement && <p role="status" className="form-error">A protected staged directory may still need manual cleanup at <code dir="ltr">{expiredPlacement}</code>. It remains owner-only. Open a new session and remove that directory with your SSH account.</p>}
    {!session && !busy && <div className="unavailable"><h3>{pendingCleanup ? "Private file cleanup needs attention." : "Your files stay on your server."}</h3><p>{pendingCleanup ? <>A restricted staged file is pending cleanup at <code dir="ltr">{pendingCleanup.location}</code>. Retry cleanup before opening another file session.</> : "Open a session to browse as your SSH user. The session locks after 5 minutes of inactivity or after 30 minutes. Leaving Files clears browsing data; queued transfers retain their session until finished."}</p></div>}
    {session && <>
      <div className="file-session"><span className="state-pill">SSH authenticated · {session.profile.user}@{session.profile.hostname}</span><small>Absolute expiry {new Date(session.absolute_expires_at).toLocaleTimeString()}</small><Secret label="Host key fingerprint" value={session.fingerprint} /></div>
      {selection && <section className="file-change-review" aria-label="Review transfer"><h4>{selection.local.direction === "upload" ? "Upload selected file" : "Download selected file"}</h4><dl><dt>Server</dt><dd>{server.name} · {session.profile.user}@{session.profile.hostname}</dd><dt>Local file</dt><dd><code>{selection.local.path_label}</code></dd><dt>Remote destination / source</dt><dd><code>{selection.local.direction === "upload" ? `${selection.resource.label.replace(/\/$/, "")}/${selection.local.name}` : selection.resource.label}</code></dd></dl><p>Overwrite disabled. Content is streamed and verified before publication. Selection expires after 10 minutes.</p><button className="primary" disabled={busy} onClick={() => void startTransfer()}>Start transfer</button><button disabled={busy} onClick={cancelSelection}>Cancel selection</button></section>}
      {placement && <section className="file-change-review protected-placement" aria-label="Protected file placement">
        <h4>Place a protected file</h4>
        <dl>
          <dt>Server</dt><dd>{server.name} · {session.profile.user}@{session.profile.hostname}</dd>
          {placement.local && <><dt>Local file</dt><dd><code>{placement.local.path_label}</code></dd><dt>File name</dt><dd>{placement.local.name}</dd></>}
          {placement.stage && <><dt>Staged file</dt><dd>{placement.stage.name} · {placement.stage.size_bytes} bytes</dd><dt>SHA-256</dt><dd><code>{placement.stage.sha256 ?? "Not verified"}</code></dd><dt>Private staging path</dt><dd><code dir="ltr">{placement.stage.staging_location}</code></dd></>}
          {placement.receipt && <><dt>Destination</dt><dd><code dir="ltr">{placement.receipt.destination}</code></dd><dt>Installed as</dt><dd>{placement.receipt.owner}:{placement.receipt.group} · mode {placement.receipt.mode}</dd><dt>Verified SHA-256</dt><dd><code>{placement.receipt.sha256}</code></dd></>}
        </dl>
        {placement.local && <><p>The selected file will be uploaded as your SSH user into a private 0700 folder with a 0600 file, then checked before any elevated action.</p><button className="primary" disabled={busy} onClick={() => void stageProtected()}>Upload to private staging</button><button disabled={busy} onClick={cancelProtectedSelection}>Cancel</button></>}
        {placement.stage && !placement.receipt && !placement.preview && !placement.stage.cleanup_needed && <>
          <label>Absolute destination<input autoFocus required maxLength={4096} aria-label="Absolute destination" placeholder="/etc/example.conf" value={placement.destination} disabled={busy} onChange={event => updatePlacement(current => current ? { ...current, destination: event.target.value } : null)} /></label>
          <label>Owner user ID<input required inputMode="numeric" pattern="[0-9]+" aria-label="Owner user ID" value={placement.owner} disabled={busy} onChange={event => updatePlacement(current => current ? { ...current, owner: event.target.value } : null)} /></label>
          <label>Group ID<input required inputMode="numeric" pattern="[0-9]+" aria-label="Group ID" value={placement.group} disabled={busy} onChange={event => updatePlacement(current => current ? { ...current, group: event.target.value } : null)} /></label>
          <label>Mode (octal)<input required inputMode="numeric" pattern="[0-7]{3,4}" aria-label="Mode (octal)" value={placement.mode} disabled={busy} onChange={event => updatePlacement(current => current ? { ...current, mode: event.target.value } : null)} /></label>
          <p>Existing destinations are never overwritten. Elevated placement requires a fresh sudo password and a root-owned parent directory that is not writable by group or others.</p>
          <button className="primary" disabled={busy} onClick={() => void reviewProtectedPlacement()}>Review protected placement</button>
          <button disabled={busy} onClick={() => void removeProtectedStage()}>Discard staged file</button>
        </>}
        {placement.stage?.cleanup_needed && !placement.receipt && <><p role="alert" className="form-error">This file could not be fully verified or cleaned. Placement is disabled until the private staging copy is removed.</p><button disabled={busy} onClick={() => void removeProtectedStage()}>Remove private stage</button></>}
        {placement.preview && <form onSubmit={event => void confirmProtectedPlacement(event)}>
          <dl><dt>Reviewed server</dt><dd>{placement.preview.server} · {placement.preview.endpoint}</dd><dt>Exact action</dt><dd><bdi dir="ltr">{placement.preview.action}</bdi></dd></dl>
          <label>Password for this action<input ref={placementPassword} type="password" aria-label="Password for this action" autoComplete="off" maxLength={1024} required disabled={busy} /></label>
          <p>The password is used for this action only. A rejected password or destination collision keeps the staged source private for a fresh review.</p>
          <button className="primary" type="submit" disabled={busy}>{busy ? "Applying reviewed placement…" : "Confirm with fresh sudo"}</button>
          <button type="button" disabled={busy} onClick={() => void cancelProtectedReview()}>Cancel review</button>
        </form>}
        {placement.receipt && <><p role="status">The destination content, owner, group, and mode were verified after publication.</p>{placement.receipt.cleanup_needed && <><p className="form-error">The destination is in place, but the restricted staged copy still needs removal from <code>{placement.receipt.staging_location}</code>.</p><button disabled={busy} onClick={() => void removeProtectedStage()}>Retry private stage cleanup</button></>}{!placement.receipt.cleanup_needed && <button disabled={busy} onClick={() => { updatePlacement(() => null); placementResource.current = null; }}>Done</button>}</>}
      </section>}
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
        <div className="file-location"><code>{page.directory.label}</code><div><button disabled={browsingDisabled} onClick={() => void pick(page.directory, false)}>Pick upload</button><button disabled={browsingDisabled} onClick={() => void pickProtected(page.directory)}>Protected placement</button><button disabled={browsingDisabled || !!pinned} onClick={() => { setFailure(""); setForm({ kind: "mkdir", resourceId: page.directory.id, name: "", preview: null }); }}>New folder</button><button disabled={browsingDisabled} onClick={() => openDirectory(page.directory.id)}>Refresh folder</button></div></div>
        <div className="file-layout"><div className="file-list"><table><thead><tr><th>Name</th><th>Type</th><th>Bytes</th></tr></thead><tbody>{page.entries.map(entry => <tr key={entry.resource.id} aria-selected={selected?.resource.id === entry.resource.id}><td><button className="text-button" disabled={browsingDisabled} onClick={() => void read({ kind: "stat", resource_id: entry.resource.id })}>{entry.resource.label}</button>{entry.kind === "directory" && <button className="file-open" disabled={browsingDisabled} aria-label={`Open folder ${entry.resource.label}`} onClick={() => openDirectory(entry.resource.id)}>Open →</button>}</td><td>{entry.kind}</td><td>{entry.size_bytes ?? "Unknown"}</td></tr>)}</tbody></table>{!page.entries.length && <p className="empty-copy">This page contains no entries.</p>}</div>
          <aside className="file-details" aria-label="Selected file">{selected ? <><h4>{selected.resource.label}</h4><dl><dt>Type</dt><dd>{selected.kind}</dd><dt>Size (bytes)</dt><dd>{selected.size_bytes ?? "Unknown"}</dd><dt>Permissions</dt><dd>{selected.permissions ?? "Unknown"}</dd><dt>Owner / group ID</dt><dd>{selected.owner ?? "?"} / {selected.group ?? "?"}</dd><dt>Modified</dt><dd>{selected.modified_at ? new Date(selected.modified_at).toLocaleString() : "Unknown"}</dd></dl>{selected.kind === "file" && <button disabled={busy || form !== null || selection !== null} onClick={() => void pick(selected.resource, true)}>Download selected file</button>}{selected.kind === "symlink" && <button disabled={browsingDisabled} onClick={() => void read({ kind: "follow_link", resource_id: selected.resource.id })}>Follow link to folder</button>}{["file", "directory", "symlink"].includes(selected.kind) && <div className="file-entry-actions"><button disabled={browsingDisabled || !!pinned} aria-label="Rename selected entry" onClick={() => setForm({ kind: "rename", resourceId: selected.resource.id, name: "", preview: null })}>Rename</button><button disabled={browsingDisabled || !!pinned} aria-label="Move selected entry" onClick={() => void read({ kind: "pin_source", resource_id: selected.resource.id })}>Move</button><button disabled={browsingDisabled || !!pinned} aria-label="Delete selected entry" onClick={() => setForm({ kind: "delete", resourceId: selected.resource.id, name: "", preview: null })}>Delete</button></div>}</> : <p>Select a name to inspect its metadata.</p>}</aside></div>
        <div className="file-page"><span>{busy ? "Reading server…" : `${page.entries.length} entries on this page`}</span><button disabled={browsingDisabled || !page.next_cursor} onClick={() => void read({ kind: "list", resource_id: page.directory.id, cursor: page.next_cursor })}>Next page</button></div>
      </>}
      <p className="muted">Uses your SSH account’s permissions. Changes require confirmation. Transfers publish only verified files and never overwrite. Recursive deletion and sudo are unavailable.</p>
    </>}
    {jobs.length > 0 && <TransferQueue rows={jobs} failure={transfers.failure} />}
  </section>;
}
