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

/** The last discovery for a server, kept on this PC so the tab can show known
 * applications at once while a new discovery runs. Display only: opening still
 * verifies SSH, and an address-specific app waits for a fresh discovery. */
export type CachedDiscovery = { discovered_at: string; complete: boolean; apps: DiscoveredApp[] };
const cacheKey = (serverId: string) => `infrawerf.apps.discovered.${serverId}`;
const ICON = /^data:image\/(png|x-icon|gif|jpeg|webp);base64,[A-Za-z0-9+/]+={0,2}$/;
const HOST = /^[0-9A-Fa-f.:]{2,45}$/;
const PROCESS = /^[A-Za-z0-9._-]{1,64}$/;
const text = (value: unknown, max: number) => typeof value === "string" && value.length <= max ? value : null;

function cleanApp(item: unknown): DiscoveredApp | null {
  if (!item || typeof item !== "object") return null;
  const app = item as Record<string, unknown>;
  const port = Number(app.port);
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
  const remote_host = app.remote_host === null || app.remote_host === undefined ? null : text(app.remote_host, 45);
  if (remote_host !== null && !HOST.test(remote_host)) return null;
  const status = app.http_status === null ? null : Number(app.http_status);
  if (status !== null && (!Number.isInteger(status) || status < 100 || status > 599)) return null;
  const process = text(app.process, 64);
  const icon = text(app.icon, 8400);
  return {
    port, bind: text(app.bind, 64) ?? "", remote_host, http_status: status, https_only: app.https_only === true,
    title: text(app.title, 120), process: process && PROCESS.test(process) ? process : null, icon: icon && ICON.test(icon) ? icon : null, version: text(app.version, 60),
  };
}

export function loadDiscovery(serverId: string, storage: Pick<Storage, "getItem"> | undefined = globalThis.localStorage): CachedDiscovery | null {
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(cacheKey(serverId)) ?? "null");
    if (!parsed || typeof parsed !== "object") return null;
    const value = parsed as Record<string, unknown>;
    const at = text(value.discovered_at, 40);
    if (!at || Number.isNaN(Date.parse(at)) || !Array.isArray(value.apps)) return null;
    return { discovered_at: at, complete: value.complete !== false, apps: value.apps.slice(0, 64).flatMap(item => { const app = cleanApp(item); return app ? [app] : []; }) };
  } catch { return null; }
}
export function saveDiscovery(serverId: string, found: CachedDiscovery, storage: Pick<Storage, "setItem"> | undefined = globalThis.localStorage) {
  try { storage?.setItem(cacheKey(serverId), JSON.stringify({ discovered_at: found.discovered_at, complete: found.complete, apps: found.apps.slice(0, 64) })); } catch { /* per-viewer convenience only */ }
}

/** Short tile name: the page title before a separator, else the process or port. */
export function appName(app: Pick<DiscoveredApp, "title" | "process" | "port">): string {
  const title = app.title?.split(/\s[|·—–-]\s|:\s/)[0]?.trim();
  return title || app.process || `Port ${app.port}`;
}
/** A stable hue for an app's letter tile. */
export function appHue(name: string): number {
  let hash = 0;
  for (const char of name.toLowerCase()) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return hash % 360;
}
