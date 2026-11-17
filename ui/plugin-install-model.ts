import type { ContributionChange, InstallRequest, InstallSource, UpdateDiff } from "./contracts.gen.ts";

/** A request from the form's values; null with a reason when something is missing. */
export function installRequest(kind: InstallSource["kind"], location: string, expected: string, key: string): InstallRequest | string {
  const where = location.trim();
  if (!where) return kind === "address" ? "Enter the https:// address of the plugin archive." : "Choose a file or folder first.";
  if (kind === "address" && !where.startsWith("https://")) return "Only https:// addresses can be installed from.";
  const hash = expected.trim();
  if (hash && !/^[0-9a-fA-F]{64}$/.test(hash)) return "An expected SHA-256 is 64 hexadecimal digits, or leave it empty.";
  const source: InstallSource = kind === "address" ? { kind, url: where } : { kind, path: where };
  return { source, ...(hash ? { expected_sha256: hash } : {}), ...(key.trim() ? { public_key: key.trim() } : {}) };
}


const CHANGE: Record<ContributionChange["change"], string> = {
  added: "New", removed: "Removed", command_changed: "Command changed", app_changed: "Application address changed",
  access_changed: "Uses different capabilities", presentation: "Title or presentation changed",
};
/** One changed contribution, in words. */
export const describeChange = (change: ContributionChange) => `${CHANGE[change.change]}: ${change.title}`;

/** Whether the plugin keeps working after the update, in one sentence. */
export function updateVerdict(diff: UpdateDiff): string {
  if (diff.needs_review) return "This update asks for something you have not reviewed. Applying it switches the plugin off until you review it and enable it again; nothing new is allowed until you allow it.";
  return "This update asks for nothing new. It keeps your review and permissions and keeps working.";
}

/** What changed about the author's signature. */
export function signerText(diff: UpdateDiff): string {
  const now = diff.signer, before = diff.previous_signer;
  if (now && before) return now === before ? `Signed by the same key as before (${now}).` : `Signed by a different key (${now}) than the installed version (${before}).`;
  if (now) return `Signed by key ${now}. The installed version's signature was not checked.`;
  if (before) return `Not checked: no public key was given. The installed version was signed by key ${before}.`;
  return "Signature not checked: no public key was given.";
}

export function updatedMessage(name: string, diff: UpdateDiff): string {
  return diff.needs_review ? `${name} updated to ${diff.to_version}. It is switched off until you review and enable it again.` : `${name} updated to ${diff.to_version}. It keeps working with the permissions you set.`;
}
