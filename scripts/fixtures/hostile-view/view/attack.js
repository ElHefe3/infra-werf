// Test fixture (T047): tries every way out of a plugin view and records
// what happened in #results. The native suite reads the results and checks
// InfraWerf's state and activity log; it never trusts these results alone.
const results = {};
const settle = (promise, ms = 3000) => Promise.race([promise, new Promise(resolve => setTimeout(() => resolve("timeout"), ms))]);
const outcome = async (name, attempt) => {
  try { results[name] = { reached: true, value: String(await settle(Promise.resolve().then(attempt))).slice(0, 200) }; }
  catch (error) { results[name] = { reached: false, value: String(error && (error.message || error)).slice(0, 200) }; }
};
const api = path => new URL(`../infrawerf/v1/${path}`, location.href);
const status = async response => { const body = await response.text(); if (!response.ok) throw new Error(`${response.status} ${body}`); return body; };

(async () => {
  try { const i = window.__TAURI_INTERNALS__; const d = i && Object.getOwnPropertyDescriptor(i, 'invoke'); results.diagnostics = { value: JSON.stringify({ present: Boolean(i), frozen: i ? Object.isFrozen(i) : null, invoke: d ? { writable: d.writable, configurable: d.configurable, get: Boolean(d.get), patched: String(d.value || '').includes('plugin view') } : null, keys: i ? Object.getOwnPropertyNames(i) : [], windowDescriptor: (() => { const w = Object.getOwnPropertyDescriptor(window, '__TAURI_INTERNALS__'); return w ? { writable: w.writable, configurable: w.configurable, get: Boolean(w.get) } : null; })() }) }; } catch (e) { results.diagnostics = { value: String(e) }; }
  // Tauri's own IPC, by every route it offers.
  await outcome("tauri_internals", () => { if (!window.__TAURI_INTERNALS__) throw new Error("absent"); return window.__TAURI_INTERNALS__.invoke("map_state"); });
  await outcome("tauri_add_server", () => { if (!window.__TAURI_INTERNALS__) throw new Error("absent"); return window.__TAURI_INTERNALS__.invoke("add_server", { input: { name: "Planted by a plugin", host: "planted.invalid" } }); });
  await outcome("tauri_post_message", () => { if (!window.__TAURI_INTERNALS__ || !window.__TAURI_INTERNALS__.postMessage) throw new Error("absent"); return window.__TAURI_INTERNALS__.postMessage({ cmd: "remove_server" }); });
  await outcome("tauri_global", () => { if (!window.__TAURI__) throw new Error("absent"); return window.__TAURI__.core.invoke("map_state"); });
  await outcome("ipc_postmessage", () => { if (!window.ipc) throw new Error("absent"); window.ipc.postMessage(JSON.stringify({ cmd: "remove_server" })); return "sent"; });
  await outcome("webview_postmessage", () => { if (!window.chrome || !window.chrome.webview) throw new Error("absent"); window.chrome.webview.postMessage("map_state"); return "sent"; });
  await outcome("ipc_protocol_fetch", () => fetch("http://ipc.localhost/map_state", { method: "POST", headers: { "Tauri-Callback": "1", "Tauri-Error": "2", "Content-Type": "application/json" }, body: "{}" }).then(status));
  await outcome("ipc_protocol_remove", () => fetch("http://ipc.localhost/remove_server", { method: "POST", body: "{}" }).then(status));
  // InfraWerf's own interface and other origins.
  await outcome("main_origin_fetch", () => fetch("http://tauri.localhost/").then(status));
  await outcome("internet_fetch", () => fetch("https://example.com/").then(status));
  await outcome("image_beacon", () => new Promise((resolve, reject) => { const image = new Image(); image.onload = () => resolve("loaded"); image.onerror = () => reject(new Error("blocked")); image.src = "https://example.com/pixel.png"; }));
  await outcome("websocket", () => new Promise((resolve, reject) => { const socket = new WebSocket("ws://127.0.0.1:9/"); socket.onopen = () => resolve("open"); socket.onerror = () => reject(new Error("blocked")); }));
  // Frames, so a fresh window might carry IPC again.
  await outcome("iframe_main", () => new Promise((resolve, reject) => { const frame = document.createElement("iframe"); frame.src = "http://tauri.localhost/"; frame.onload = () => { try { resolve(frame.contentDocument ? "readable" : "loaded-opaque"); } catch (e) { reject(e); } }; document.body.append(frame); setTimeout(() => reject(new Error("no load")), 2000); }));
  await outcome("blank_frame_ipc", () => { const frame = document.createElement("iframe"); document.body.append(frame); const w = frame.contentWindow; if (w.__TAURI_INTERNALS__) return w.__TAURI_INTERNALS__.invoke("map_state"); if (w.chrome && w.chrome.webview) { w.chrome.webview.postMessage("map_state"); return "sent"; } throw new Error("absent"); });
  await outcome("worker", () => new Promise((resolve, reject) => { try { const worker = new Worker(URL.createObjectURL(new Blob(["postMessage(1)"]))); worker.onmessage = () => resolve("ran"); worker.onerror = () => reject(new Error("blocked")); } catch (error) { reject(error); } }));
  // Leaving the view.
  await outcome("window_open", () => { const w = window.open("http://tauri.localhost/"); if (!w) throw new Error("refused"); return "opened"; });
  await outcome("form_post", () => { const form = document.createElement("form"); form.method = "POST"; form.action = "http://ipc.localhost/remove_server"; document.body.append(form); form.submit(); return "submitted"; });
  // The message API: without the secret, and for data not granted.
  await outcome("api_without_secret", () => fetch(`${location.origin}/infrawerf/v1/context`).then(status));
  await outcome("api_context", () => fetch(api("context")).then(status));
  await outcome("api_observations", () => fetch(api("observations")).then(status));
  await outcome("api_inventory", () => fetch(api("inventory")).then(status));
  await outcome("api_action", () => fetch(api("actions"), { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ contribution_id: "kernel" }) }).then(status));
  await outcome("api_unknown", () => fetch(api("servers/remove")).then(status));
  document.getElementById("results").textContent = JSON.stringify(results);
  document.title = "done";
  // Last: try to navigate the whole view away to InfraWerf's own page.
  setTimeout(() => { location.href = "http://tauri.localhost/"; }, 300);
})();
