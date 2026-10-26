import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { FileSession, MapServer } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
const client = createWorkspaceClient();

/** One native protected view. Unmount/Lock cancels its work and clears results. */
export function useProtectedSession(server: MapServer, onBusy: (busy: boolean) => void) {
  const [session, setSession] = useState<FileSession | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [notice, setNotice] = useState("");
  const active = useRef<FileSession | null>(null), generation = useRef(0), operation = useRef<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined), starting = useRef(false);
  function lock(message = "Session locked. Protected results have been cleared.") {
    const value = active.current, pending = operation.current;
    generation.current++; active.current = null; operation.current = null; starting.current = false;
    clearTimeout(timer.current); setSession(null); setBusy(false); onBusy(false); setError(""); setNotice(message);
    if (pending) void client.cancelSsh(pending).catch(() => {});
    if (value) void client.lockFiles(value.session_id).catch(() => setError("Session cleanup could not be confirmed. Wait for expiry before closing InfraWerf."));
  }
  function arm(value: FileSession) {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => lock("Session expired. Open a new session to continue."), Math.max(0, Math.min(value.idle_seconds * 1000, Date.parse(value.absolute_expires_at) - Date.now())));
  }
  async function connect() {
    if (starting.current || active.current) return;
    starting.current = true; const epoch = ++generation.current;
    setBusy(true); onBusy(true); setError(""); setNotice("");
    try {
      const ticket = await client.prepareFiles(server.id);
      if (generation.current !== epoch) { await client.cancelSsh(ticket.operation_id); return; }
      operation.current = ticket.operation_id;
      const value = await client.openFiles(server.id, ticket.operation_id);
      if (generation.current !== epoch) { await client.lockFiles(value.session_id); return; }
      active.current = value; operation.current = null; setSession(value); arm(value);
    } catch (failure) { if (epoch === generation.current) setError(errorMessage(failure)); }
    finally { if (epoch === generation.current) { starting.current = false; setBusy(false); onBusy(false); operation.current = null; } }
  }
  useEffect(() => {
    let disposed = false; const listeners: (() => void)[] = [];
    const keep = (fn: () => void) => { if (disposed) fn(); else listeners.push(fn); };
    void listen<string | null>("atlas-session-closed", event => { if (active.current && (event.payload === null || event.payload === active.current.session_id)) lock("Session closed. Protected results have been cleared."); }).then(keep).catch(() => {});
    void listen<string>("atlas-close-blocked", event => setError(event.payload)).then(keep).catch(() => {});
    const hide = () => { if (document.visibilityState === "hidden") lock("Session locked while InfraWerf was hidden."); };
    document.addEventListener("visibilitychange", hide);
    return () => {
      generation.current++; clearTimeout(timer.current); const value = active.current, pending = operation.current;
      active.current = null; operation.current = null;
      if (value) void client.lockFiles(value.session_id).catch(() => {});
      if (pending) void client.cancelSsh(pending).catch(() => {});
      disposed = true; document.removeEventListener("visibilitychange", hide); listeners.forEach(fn => { try { fn(); } catch {} }); onBusy(false);
    };
  }, []);
  return { session, active, generation, busy, error, notice, connect, lock, arm };
}
