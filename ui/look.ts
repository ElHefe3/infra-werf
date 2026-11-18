// Visual look (classic or liquid glass) and light/dark theme of the shell.
// Per-PC display preferences only: kept in this webview's storage and never
// sent to the Rust core or a server.
import { useEffect, useState } from "react";

export type Look = "classic" | "glass";
export type Theme = "light" | "dark";
const LOOK_KEY = "infrawerf.look";
const THEME_KEY = "infrawerf.theme";

type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

function read(key: string, storage: Storage | undefined): string | null {
  try { return storage?.getItem(key) ?? null; } catch { return null; }
}
function write(key: string, value: string, storage: Storage | undefined) {
  try { storage?.setItem(key, value); } catch { /* It still applies for this run. */ }
}

export function loadLook(storage: Storage | undefined = globalThis.localStorage): Look {
  return read(LOOK_KEY, storage) === "classic" ? "classic" : "glass";
}
export function saveLook(look: Look, storage: Storage | undefined = globalThis.localStorage) { write(LOOK_KEY, look, storage); }

/** A saved choice wins; otherwise follow Windows' light or dark app mode. */
export function loadTheme(storage: Storage | undefined = globalThis.localStorage, systemDark = prefersDark()): Theme {
  const saved = read(THEME_KEY, storage);
  return saved === "dark" || saved === "light" ? saved : systemDark ? "dark" : "light";
}
export function saveTheme(theme: Theme, storage: Storage | undefined = globalThis.localStorage) { write(THEME_KEY, theme, storage); }

function prefersDark(): boolean {
  try { return globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false; } catch { return false; }
}

export function useLook(): { look: Look; theme: Theme; toggleLook: () => void; toggleTheme: () => void } {
  const [look, setLook] = useState<Look>(() => loadLook());
  const [theme, setTheme] = useState<Theme>(() => loadTheme());
  useEffect(() => { document.documentElement.dataset.look = look; saveLook(look); }, [look]);
  useEffect(() => { document.documentElement.dataset.theme = theme; document.documentElement.style.colorScheme = theme; }, [theme]);
  return {
    look, theme,
    toggleLook: () => setLook(current => current === "glass" ? "classic" : "glass"),
    toggleTheme: () => setTheme(current => { const next = current === "dark" ? "light" : "dark"; saveTheme(next); return next; }),
  };
}

