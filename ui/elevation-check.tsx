import { useEffect, useRef, useState } from "react";
import type { ElevationPreview, MapServer } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { useProtectedSession } from "./protected-session";
import { errorMessage } from "./map-model";
const client = createWorkspaceClient();
export function ElevationCheck({ server, onBusy, disabled = false }: { server: MapServer; onBusy: (value: boolean) => void; disabled?: boolean }) {
  const access = useProtectedSession(server, onBusy);
  const [preview, setPreview] = useState<ElevationPreview | null>(null), [busy, setBusy] = useState(false), [error, setError] = useState(""), [notice, setNotice] = useState("");
  const password = useRef<HTMLInputElement>(null), pending = useRef<string | null>(null), working = useRef(false);
  function clear() { if (password.current) password.current.value = ""; setPreview(null); setNotice(""); setError(""); pending.current = null; working.current = false; setBusy(false); }
  useEffect(() => { clear(); return () => { if (password.current) password.current.value = ""; }; }, [access.session?.session_id]);
  useEffect(() => {
    if (!preview) return;
    const id = preview.operation_id;
    const timer = setTimeout(() => {
      if (working.current) return;
      const session = access.active.current;
      if (session) void client.cancelElevation(server.id, session.session_id, id).catch(() => {});
      clear(); setNotice("This review expired. Review the action again before entering a password.");
    }, Math.max(0, Date.parse(preview.expires_at) - Date.now()));
    return () => clearTimeout(timer);
  }, [preview?.operation_id]);
  async function review() {
    const session = access.active.current; if (!session || working.current) return;
    working.current = true; setBusy(true); setError(""); setNotice(""); const epoch = access.generation.current;
    try {
      const value = await client.previewElevation(server.id, session.session_id);
      if (epoch !== access.generation.current) { await client.cancelElevation(server.id, session.session_id, value.operation_id).catch(() => {}); return; }
      pending.current = value.operation_id; setPreview(value); access.arm(session);
    } catch (failure) { if (epoch === access.generation.current) setError(errorMessage(failure)); }
    finally { if (epoch === access.generation.current) { working.current = false; setBusy(false); } }
  }
  async function confirm(event: React.FormEvent) {
    event.preventDefault(); const session = access.active.current;
    if (!session || !preview || working.current || !password.current?.value) return;
    working.current = true; setBusy(true); onBusy(true); setError(""); setNotice(""); const epoch = access.generation.current;
    const request = client.confirmElevation(server.id, session.session_id, preview.operation_id, password.current.value);
    password.current.value = "";
    try {
      await request;
      if (epoch === access.generation.current) { setNotice("Fresh sudo check verified. Review and enter a password separately for each action."); access.arm(session); }
    } catch (failure) {
      if (epoch !== access.generation.current) return;
      const code = (failure as { code?: string })?.code;
      if (["expired_session", "io_failure", "host_trust_failure", "timed_out", "output_limit"].includes(code ?? "")) access.lock();
      setError(code === "unauthenticated" ? "The fresh password check failed. A new action needs a new review and password." : code === "denied" ? "The server did not allow a fresh password check. Review its access policy before trying again." : errorMessage(failure));
    } finally { if (epoch === access.generation.current) { pending.current = null; setPreview(null); working.current = false; setBusy(false); onBusy(false); } }
  }
  async function cancel() {
    if (password.current) password.current.value = "";
    const session = access.active.current, operation = pending.current;
    if (session && operation) try { await client.cancelElevation(server.id, session.session_id, operation); } catch (failure) { setError(errorMessage(failure)); return; }
    if (!working.current) clear(); else setNotice("Stopping the action…");
  }
  return <section className="ssh-connection" aria-label="Fresh elevation check"><div className="section-heading"><div><h3>Protected access & elevation</h3><p>Check a fresh sudo password and root identity without changing files or services.</p></div>{access.session ? <button onClick={() => access.lock()}>Lock protected session</button> : access.busy ? <button onClick={() => access.lock("Connection cancelled.")}>Cancel protected connection</button> : <button disabled={busy || disabled} className="primary" onClick={() => void access.connect()}>Open protected session</button>}</div>
    {(access.error || error) && <p role="alert" className="form-error">{access.error || error}</p>}
    {(access.notice || notice) && <p role="status">{notice || access.notice}</p>}
    {access.session && <><div className="file-session"><span className="state-pill">SSH authenticated · {access.session.profile.user}@{access.session.profile.hostname}</span><code>{access.session.fingerprint}</code></div>
      {!preview && <button disabled={busy || disabled} onClick={() => void review()}>Review elevated access check</button>}
      {preview && <form className="file-change-review" aria-label="Review elevated action" onSubmit={event => void confirm(event)}><dl><dt>Server</dt><dd>{server.name} · {preview.server}</dd><dt>Endpoint</dt><dd>{preview.endpoint}</dd><dt>Action</dt><dd>{preview.action}</dd></dl><label>Password for this action<input ref={password} type="password" aria-label="Password for this action" autoComplete="off" maxLength={1024} required disabled={busy} /></label><p>The password stays within this action. Cancelling or locking requires a new review before another attempt.</p><button className="primary" type="submit" disabled={busy}>{busy ? "Checking fresh access…" : "Confirm with fresh sudo"}</button><button type="button" onClick={() => void cancel()}>{busy ? "Cancel elevated action" : "Cancel elevation review"}</button></form>}
    </>}
  </section>;
}
