import test from "node:test";
import assert from "node:assert/strict";
import { loadLook, loadTheme, saveLook, saveTheme } from "./look.ts";

test("the look defaults to glass, keeps a saved choice and survives blocked storage", () => {
  const store = new Map<string, string>();
  const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v) };
  assert.equal(loadLook(storage), "glass");
  saveLook("classic", storage);
  assert.equal(loadLook(storage), "classic");
  store.set("infrawerf.look", "neon");
  assert.equal(loadLook(storage), "glass");
  const blocked = { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } };
  assert.equal(loadLook(blocked), "glass");
  assert.doesNotThrow(() => saveLook("glass", blocked));
});

test("the theme follows the system until a choice is saved", () => {
  const store = new Map<string, string>();
  const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v) };
  assert.equal(loadTheme(storage, false), "light");
  assert.equal(loadTheme(storage, true), "dark");
  saveTheme("light", storage);
  assert.equal(loadTheme(storage, true), "light");
  store.set("infrawerf.theme", "purple");
  assert.equal(loadTheme(storage, true), "dark");
});
