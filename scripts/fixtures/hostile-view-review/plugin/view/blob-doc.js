// Runs inside a blob: document the view navigated itself to.
(async () => {
  window.REVIEW_TCP = window.REVIEW && window.REVIEW.tcp;
  const out = { href: location.href.slice(0, 60), inline_script_ran: Boolean(window.INLINE_RAN),webview: Boolean(window.chrome && window.chrome.webview), internals: Boolean(window.__TAURI_INTERNALS__), ipc: Boolean(window.ipc) };
  const settle = (p, ms = 3000) => Promise.race([p, new Promise(r => setTimeout(() => r("timeout"), ms))]);
  try { out.invoke = String(await settle(window.__TAURI_INTERNALS__.invoke("map_state").then(v => "answered " + JSON.stringify(v).slice(0, 80), e => "rejected " + String(e).slice(0, 120)))); } catch (e) { out.invoke = "threw " + String(e).slice(0, 120); }
  try { if (window.chrome && window.chrome.webview) { window.chrome.webview.postMessage("x"); out.post = "sent"; } } catch (e) { out.post = String(e); }
  try { out.external = String(await settle(fetch(`http://127.0.0.1:${window.REVIEW_TCP}/from-blob`, { mode: "no-cors" }).then(() => "reached", e => "blocked " + e))); } catch (e) { out.external = String(e); }
  document.title = "blob:" + JSON.stringify(out);
})();
