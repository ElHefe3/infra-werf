import React, { useState } from "react";
import type { InstallProblem, InstallProblemCode, InstallRecordView, InstallReport, InstallRequest, InstallSource, UpdatePreview } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import { date } from "./workspace-parts";
import { describeChange, installRequest, signerText, updatedMessage, updateVerdict } from "./plugin-install-model";
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
  different_plugin: "Another plugin", downgrade: "An older version", changed_since_preview: "Changed since you checked", preview_expired: "Checked too long ago",
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

/** Where a plugin comes from: shared by installing and updating. */
function SourceForm({ label, title, intro, action, working: workingLabel, busy, submit }: { label: string; title: string; intro: string; action: string; working: string; busy: boolean; submit: (request: InstallRequest) => Promise<string | null> }) {
  const [kind, setKind] = useState<Kind>("archive");
  const [location, setLocation] = useState(""), [expected, setExpected] = useState(""), [key, setKey] = useState("");
  const [working, setWorking] = useState(false), [failure, setFailure] = useState("");
  const locked = busy || working;
  async function choose() {
    setFailure("");
    try { const picked = await client.pickPluginSource(kind === "folder"); if (picked) setLocation(picked); }
    catch (error) { setFailure(errorMessage(error)); }
  }
  async function send(event: React.FormEvent) {
    event.preventDefault();
    const request = installRequest(kind, location, expected, key);
    if (typeof request === "string") { setFailure(request); return; }
    setWorking(true); setFailure("");
    try {
      const problem = await submit(request);
      if (problem) setFailure(problem); else { setLocation(""); setExpected(""); setKey(""); }
    } catch (error) { setFailure(errorMessage(error)); }
    finally { setWorking(false); }
  }
  return <form className="plugin-install" aria-label={label} onSubmit={send}>
    <h5>{title}</h5>
    <p className="muted">{intro}</p>
    <fieldset className="install-kind"><legend>From</legend>{KINDS.map(option => <label key={option.value}><input type="radio" name={`${label}-kind`} value={option.value} checked={kind === option.value} disabled={locked} onChange={() => { setKind(option.value); setLocation(""); setFailure(""); }} />{option.label}</label>)}</fieldset>
    <p className="muted">{KINDS.find(option => option.value === kind)?.hint}</p>
    {kind === "address"
      ? <label>Address<input aria-label="Plugin address" type="url" spellCheck={false} placeholder="https://example.org/my-plugin.zip" value={location} disabled={locked} onChange={e => setLocation(e.target.value)} /></label>
      : <div className="install-pick"><input aria-label={kind === "folder" ? "Plugin folder" : "Plugin archive"} readOnly value={location} placeholder="Nothing chosen" /><button type="button" disabled={locked} onClick={() => void choose()}>{kind === "folder" ? "Choose folder…" : "Choose file…"}</button></div>}
    <details className="install-checks"><summary>Check it first (optional)</summary>
      <label>Expected SHA-256<input aria-label="Expected SHA-256" spellCheck={false} maxLength={64} placeholder={kind === "folder" ? "SHA-256 of the plugin's files" : "SHA-256 of the archive"} value={expected} disabled={locked} onChange={e => setExpected(e.target.value)} /></label>
      {kind !== "folder" && <label>Author's minisign public key<input aria-label="Author public key" spellCheck={false} placeholder="RW…" value={key} disabled={locked} onChange={e => setKey(e.target.value)} /><small>When given, the archive must have a valid signature by this key next to it ({kind === "address" ? "the same address with .minisig added" : "the same file name with .minisig added"}).</small></label>}
    </details>
    {failure && <p className="form-error" role="alert">{failure}</p>}
    <button className="primary" disabled={locked}>{working ? workingLabel : action}</button>
  </form>;
}

export const problemText = (problem: InstallProblem, after: string) => `${PROBLEM[problem.code]}. ${problem.message} ${after}`;

export function InstallForm({ busy, installed }: { busy: boolean; installed: (report: InstallReport) => void }) {
  return <SourceForm label="Install a plugin" title="Install a plugin" busy={busy} action="Install" working="Checking and installing…"
    intro="Installing copies the plugin into the plugins folder after checking every file. It allows nothing: you review and enable it on its page. No install script is ever run."
    submit={async request => { const report = await client.installPlugin(request); if (report.problem) return problemText(report.problem, "Nothing was installed."); installed(report); return null; }} />;
}

/** Update an installed plugin: check first, see what changes, then apply (T046). */
export function UpdatePanel({ pluginId, name, busy, updated }: { pluginId: string; name: string; busy: boolean; updated: (message: string) => void }) {
  const [preview, setPreview] = useState<UpdatePreview | null>(null);
  const [downgrade, setDowngrade] = useState(false), [working, setWorking] = useState(false), [failure, setFailure] = useState("");
  async function apply() {
    if (!preview?.token) return;
    setWorking(true); setFailure("");
    try {
      const result = await client.applyPluginUpdate(preview.token, downgrade);
      if (result.problem) { setFailure(problemText(result.problem, "The installed version is unchanged.")); setPreview(null); }
      else if (result.diff) { setPreview(null); updated(updatedMessage(name, result.diff)); }
    } catch (error) { setFailure(errorMessage(error)); }
    finally { setWorking(false); }
  }
  function discard() { if (preview?.token) void client.discardPluginUpdate(preview.token).catch(() => {}); setPreview(null); setDowngrade(false); }
  const diff = preview?.diff;
  return <section className="plugin-update" aria-label={`Update ${name}`}>
    {!diff && <SourceForm label={`Update source for ${name}`} title="Update" busy={busy || working} action="Check update" working="Checking…"
      intro="Choose the new version. InfraWerf checks it like an install and shows what changes before anything is replaced. The installed version keeps working until you apply it."
      submit={async request => { setFailure(""); const found = await client.checkPluginUpdate(pluginId, request); if (found.problem) return problemText(found.problem, "The installed version is unchanged."); setPreview(found); setDowngrade(false); return null; }} />}
    {failure && <p className="form-error" role="alert">{failure}</p>}
    {diff && <div className="update-diff" role="group" aria-label="What the update changes">
      <h5>Version {diff.from_version} → {diff.to_version}</h5>
      <p className={diff.needs_review ? "form-error" : "update-ok"}>{updateVerdict(diff)}</p>
      {diff.downgrade && <label className="update-downgrade"><input type="checkbox" checked={downgrade} onChange={e => setDowngrade(e.target.checked)} /> {`I want to go back to the older version ${diff.to_version}.`}</label>}
      {diff.added_capabilities.length > 0 && <p><strong>Asks for more:</strong> {diff.added_capabilities.map(c => <code key={c}>{c}</code>)} <span className="muted">Never granted by an update: you allow it per server after reviewing.</span></p>}
      {diff.removed_capabilities.length > 0 && <p><strong>No longer asks for:</strong> {diff.removed_capabilities.map(c => <code key={c}>{c}</code>)} <span className="muted">Its permissions are removed.</span></p>}
      {diff.contributions.length > 0 && <ul className="update-changes" aria-label="Changed contributions">{diff.contributions.map(change => <li key={change.id}>{describeChange(change)}{change.command && <code>{change.command}</code>}</li>)}</ul>}
      <p className="muted">{signerText(diff)}</p>
      <dl className="plugin-facts"><div><dt>New files SHA-256</dt><dd><code aria-label="New files SHA-256">{diff.content_sha256}</code></dd></div>{diff.archive_sha256 && <div><dt>New archive SHA-256</dt><dd><code>{diff.archive_sha256}</code></dd></div>}</dl>
      <div className="plugin-actions"><button disabled={working} onClick={discard}>Cancel</button><button className="primary" disabled={working || (diff.downgrade && !downgrade)} onClick={() => void apply()}>{working ? "Applying…" : diff.needs_review ? "Apply and review again" : "Apply update"}</button></div>
    </div>}
  </section>;
}
