import type { DiscoveredApp } from "./contracts.gen";
/** Per-server application shortcuts: a name, port and path only, never secrets. */
export type AppShortcut = { name: string; port: number; path: string };
const PATH = /^\/(?!\/)[A-Za-z0-9\/\-._~%!$&'()*+,;=:@?#]*$/;

export function parseTarget(port: string, path: string): { port: number; path: string } | null {
  const value = Number(port.trim());
  const cleanPath = path.trim() || "/";
  if (!/^\d{1,5}$/.test(port.trim()) || value < 1 || value > 65535) return null;
  if (cleanPath.length > 512 || !PATH.test(cleanPath)) return null;
  return { port: value, path: cleanPath };
}

const key = (serverId: string) => `infrawerf.apps.${serverId}`;
export function loadShortcuts(serverId: string, storage: Pick<Storage, "getItem"> | undefined = globalThis.localStorage): AppShortcut[] {
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(key(serverId)) ?? "[]");
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap(item => {
      if (!item || typeof item.name !== "string" || !item.name.trim()) return [];
      const target = parseTarget(String(item.port), String(item.path));
      return target ? [{ name: item.name.slice(0, 60), ...target }] : [];
    }).slice(0, 24);
  } catch { return []; }
}
export function saveShortcuts(serverId: string, shortcuts: AppShortcut[], storage: Pick<Storage, "setItem"> | undefined = globalThis.localStorage) {
  try { storage?.setItem(key(serverId), JSON.stringify(shortcuts.slice(0, 24))); } catch { /* per-viewer convenience only */ }
}

/** Dropdown label for a discovered application, plus whether it can open. */
export function describeApp(app: DiscoveredApp): { label: string; openable: boolean } {
  const name = app.title || app.process || `Port ${app.port}`;
  const where = app.remote_host ? `${app.remote_host}:${app.port}` : `localhost:${app.port}`;
  if (app.https_only) return { label: `${name} — ${where} (HTTPS only, not supported yet)`, openable: false };
  if (app.http_status === null) return { label: `${name} — ${where} (not a web page)`, openable: false };
  if (app.http_status === 400) return { label: `${name} — ${where} (needs HTTPS or a host name)`, openable: false };
  return { label: `${name} — ${where}`, openable: true };
}
export const appKey = (app: Pick<DiscoveredApp, "port" | "remote_host">) => `${app.remote_host ?? "localhost"}:${app.port}`;
