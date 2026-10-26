import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { MapServer, SshVerification } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";

const client = createWorkspaceClient();
const labels: Record<SshVerification["state"], [string, string]> = {
  verified: ["SSH access verified", "OpenSSH verified this host and authenticated an existing identity. This check has finished; no persistent SSH session is open."],
  unknown_host: ["Host identity needs verification", "This host key is unknown. Verify the fingerprint independently, resolve trust in your existing SSH client, then retry. Atlas will not accept it automatically."],
  changed_host: ["Host identity has changed", "Stop and verify this fingerprint independently. Resolve the changed key through your existing SSH trust workflow before retrying."],
  authentication_failed: ["SSH authentication failed", "No configured public-key identity authenticated. Check your existing identity configuration or unlock/load your existing SSH agent, then retry."],
  unreachable: ["SSH server could not be reached", "Check the configured hostname, port and LAN/VPN connection. This does not establish the state of services on that host."],
  unsupported_configuration: ["SSH configuration needs attention", "Use a direct, concrete alias with a valid hostname, username and port. Jump/proxy/known-host helper configurations are unavailable in this build."],
  client_unavailable: ["System OpenSSH is unavailable", "Atlas could not find the Windows OpenSSH client. Install or restore the system client before retrying."],
  failed: ["SSH verification did not complete", "The SSH check did not produce a verified response. Review your existing SSH configuration and retry."],
};

export function SshConnection({ server, onBusy }: { server: MapServer; onBusy: (busy: boolean) => void }) {
  const [result, setResult] = useState<SshVerification | null>(null);
  const [busy, setBusy] = useState(false);
  const [cancellable, setCancellable] = useState(false);
  const [error, setError] = useState("");
  const operation = useRef<string | null>(null);
  const starting = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    let disposed = false; let unlisten: (() => void) | undefined;
    void listen<string>("atlas-close-blocked", event => setError(event.payload)).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(() => {});
    return () => { mounted.current = false; disposed = true; unlisten?.(); if (operation.current) void client.cancelSsh(operation.current).catch(() => {}); };
  }, []);
  async function verify() {
    if (starting.current) return;
    starting.current = true; setBusy(true); onBusy(true); setError(""); setResult(null);
    try {
      const ticket = await client.prepareSsh(server.id);
      operation.current = ticket.operation_id;
      setCancellable(true);
      if (!mounted.current) { await client.cancelSsh(ticket.operation_id); return; }
      const value = await client.verifySsh(server.id, ticket.operation_id);
      if (mounted.current) setResult(value);
    } catch (failure) { if (mounted.current) setError(errorMessage(failure)); }
    finally { operation.current = null; starting.current = false; if (mounted.current) { setBusy(false); setCancellable(false); onBusy(false); } }
  }
  async function cancel() {
    const id = operation.current;
    if (!id) return;
    setCancellable(false);
    try { await client.cancelSsh(id); }
    catch (failure) { if (mounted.current && operation.current === id) setError(errorMessage(failure)); }
  }
  const copy = result ? labels[result.state] : null;
  return <section className="ssh-connection" aria-label="SSH connection verification">
    <div className="section-heading"><div><h3>SSH connection</h3><p>Uses this PC’s existing OpenSSH configuration and identities.</p></div><div className="ssh-actions"><button className="primary" disabled={busy} onClick={() => void verify()}>{busy ? "Verifying SSH…" : "Verify SSH access"}</button>{busy && <button disabled={!cancellable} onClick={() => void cancel()}>Cancel check</button>}</div></div>
    <p className="muted">Alias: <strong>{server.host}</strong> · Strict host checking · No trust-file changes</p>
    <div aria-live="polite">
      {error && <p role="alert" className="form-error">{error}</p>}
      {copy && result && <div className={`ssh-result ${result.state === "verified" ? "verified" : "attention"}`} data-ssh-state={result.state}>
        <h4>{copy[0]}</h4><p>{copy[1]}</p>
        {result.profile && <dl><div><dt>Resolved endpoint</dt><dd>{result.profile.user}@{result.profile.hostname}:{result.profile.port}</dd></div><div><dt>Configured identity references</dt><dd>{result.profile.identity_files}</dd></div></dl>}
        {result.fingerprint && <div className="ssh-fingerprint"><span>{result.state === "verified" ? "Verified host fingerprint" : "Presented fingerprint · verify independently"}</span><code>{result.key_type} {result.fingerprint}</code></div>}
        <small>Checked {new Date(result.checked_at).toLocaleString()}</small>
      </div>}
    </div>
    <p className="muted">Open a protected session in Files to browse. Terminal, service controls and sudo remain unavailable.</p>
  </section>;
}
