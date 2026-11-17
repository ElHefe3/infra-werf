import type { InstallRequest, InstallSource } from "./contracts.gen.ts";

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

