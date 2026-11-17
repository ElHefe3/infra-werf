import React, { useState } from "react";
import type { InstallProblemCode, InstallRecordView, InstallReport, InstallSource } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { date } from "./workspace-parts";
import { installRequest } from "./plugin-install-model";
const client = createWorkspaceClient();

// Installing, verifying and removing plugins (S12/F22/T045). The Rust core
// reads, checks and publishes; this form only says where from.

type Kind = InstallSource["kind"];
const KINDS: { value: Kind; label: string; hint: string }[] = [
  { value: "archive", label: "Archive file", hint: "A .zip or .tar.gz file on this PC." },
  { value: "address", label: "HTTPS address", hint: "Only this exact address is fetched, once, when you press Install. Nothing else is downloaded." },
  { value: "folder", label: "Folder", hint: "A folder on this PC that holds infrawerf-plugin.json. Its files are copied; the original is left as it is." },
];

const PROBLEM: Record<InstallProblemCode, string> = {
  unreadable: "Could not be read", not_https: "Not an https:// address", unsupported_archive: "Not a .zip or .tar.gz file",
  path_traversal: "A path would leave the plugin's folder", unsafe_entry: "Holds a link or special file", too_large: "Too large",
  no_manifest: "No plugin manifest", invalid_manifest: "The manifest is not valid", hash_mismatch: "The SHA-256 is not the one you expected",
  signature_missing: "No signature", signature_invalid: "The signature does not match", invalid_public_key: "Not a minisign public key",
  signature_not_possible: "A folder cannot be signed", already_installed: "Already installed",
};

/** What the start-up check found, in words. */
export function integrityLabel(record: InstallRecordView): string {
  if (record.integrity === "changed") return "Files changed since it was installed. Nothing it asks for is allowed. Remove it and install it again.";
  if (record.integrity === "verified") return record.checked_at ? `Files match. Checked ${date(record.checked_at)}.` : "Files match.";
  return "Not checked yet: checked at the next start-up.";
}

export function InstallRecord({ record }: { record: InstallRecordView }) {
  const from = record.source_kind === "address" ? "Downloaded from" : record.source_kind === "archive" ? "Installed from the archive" : "Copied from the folder";
  return <dl className="plugin-facts plugin-install-record" aria-label="Installation">
    <div><dt>Installed</dt><dd>{date(record.installed_at)} · {from} <code>{record.source}</code></dd></div>
    <div><dt>Files SHA-256</dt><dd><code aria-label="Files SHA-256">{record.content_sha256}</code></dd></div>
    {record.archive_sha256 && <div><dt>Archive SHA-256</dt><dd><code aria-label="Archive SHA-256">{record.archive_sha256}</code></dd></div>}
    <div><dt>Signature</dt><dd>{record.signer_key_id ? <>Verified with the public key you gave, key ID <code aria-label="Signer key ID">{record.signer_key_id}</code></> : "Not checked: no public key was given"}</dd></div>
    <div><dt>Integrity</dt><dd className={`integrity ${record.integrity}`}>{integrityLabel(record)}</dd></div>
  </dl>;
}

export function InstallForm({ busy, installed }: { busy: boolean; installed: (report: InstallReport) => void }) {
  const [kind, setKind] = useState<Kind>("archive");
  const [location, setLocation] = useState(""), [expected, setExpected] = useState(""), [key, setKey] = useState("");
  const [working, setWorking] = useState(false), [failure, setFailure] = useState("");
  const locked = busy || working;
  async function choose() {
    setFailure("");
    try { const picked = await client.pickPluginSource(kind === "folder"); if (picked) setLocation(picked); }
    catch (error) { setFailure(errorMessage(error)); }
  }
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const request = installRequest(kind, location, expected, key);
    if (typeof request === "string") { setFailure(request); return; }
    setWorking(true); setFailure("");
    try {
      const report = await client.installPlugin(request);
      if (report.problem) setFailure(`${PROBLEM[report.problem.code]}. ${report.problem.message} Nothing was installed.`);
      else { setLocation(""); setExpected(""); setKey(""); installed(report); }
    } catch (error) { setFailure(errorMessage(error)); }
    finally { setWorking(false); }
  }
  return <form className="plugin-install" aria-label="Install a plugin" onSubmit={submit}>
    <h5>Install a plugin</h5>
    <p className="muted">Installing copies the plugin into the plugins folder after checking every file. It allows nothing: you review and enable it on its page. No install script is ever run.</p>
    <fieldset className="install-kind"><legend>From</legend>{KINDS.map(option => <label key={option.value}><input type="radio" name="install-kind" value={option.value} checked={kind === option.value} disabled={locked} onChange={() => { setKind(option.value); setLocation(""); setFailure(""); }} />{option.label}</label>)}</fieldset>
    <p className="muted">{KINDS.find(option => option.value === kind)?.hint}</p>
    {kind === "address"
      ? <label>Address<input aria-label="Plugin address" type="url" spellCheck={false} placeholder="https://example.org/my-plugin.zip" value={location} disabled={locked} onChange={e => setLocation(e.target.value)} /></label>
      : <div className="install-pick"><input aria-label={kind === "folder" ? "Plugin folder" : "Plugin archive"} readOnly value={location} placeholder="Nothing chosen" /><button type="button" disabled={locked} onClick={() => void choose()}>{kind === "folder" ? "Choose folder…" : "Choose file…"}</button></div>}
    <details className="install-checks"><summary>Check it before installing (optional)</summary>
      <label>Expected SHA-256<input aria-label="Expected SHA-256" spellCheck={false} maxLength={64} placeholder={kind === "folder" ? "SHA-256 of the plugin's files" : "SHA-256 of the archive"} value={expected} disabled={locked} onChange={e => setExpected(e.target.value)} /></label>
      {kind !== "folder" && <label>Author's minisign public key<input aria-label="Author public key" spellCheck={false} placeholder="RW…" value={key} disabled={locked} onChange={e => setKey(e.target.value)} /><small>When given, the archive must have a valid signature by this key next to it ({kind === "address" ? "the same address with .minisig added" : "the same file name with .minisig added"}).</small></label>}
    </details>
    {failure && <p className="form-error" role="alert">{failure}</p>}
    <button className="primary" disabled={locked}>{working ? "Checking and installing…" : "Install"}</button>
  </form>;
}
