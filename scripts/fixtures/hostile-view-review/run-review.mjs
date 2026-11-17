// T047 isolation review runner (reviewer's proof of concept, not a product
// suite). Starts InfraWerf with the review fixture, local TCP and UDP
// listeners that stand in for "another origin", and drives the fixture's
// phases. Results go to <work>/review-result.json. No SSH fixture is needed:
// nothing here runs a command on a server.
//   ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe \
//     mise run exec -- node scripts/fixtures/hostile-view-review/run-review.mjs
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { cp, mkdir, writeFile } from 'node:fs/promises';
import dgram from 'node:dgram';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const root = process.cwd();
// On C: (the temporary folder) so NTFS short names exist, as they do for a
// default plugins folder under the user's profile.
const work = path.join(os.tmpdir(), `infrawerf-t047-review-${randomUUID()}`);
await mkdir(work);
const plugins = path.join(work, 'plugins');
// An empty SSH config, so no server from the user's own config is imported.
await writeFile(path.join(work, 'ssh_config'), '');
const pluginDir = path.join(plugins, 'hostile-view-review');
await cp(path.join(root, 'scripts/fixtures/hostile-view-review/plugin'), pluginDir, { recursive: true });
const ID = 'org.example.hostile-view-review';
const delay = ms => new Promise(r => setTimeout(r, ms));
const report = { work, hits: { tcp: [], udp: [], udp2: [] } };

// Listeners standing in for other origins.
const tcp = net.createServer(socket => {
  const entry = { at: Date.now(), first: '' };
  report.hits.tcp.push(entry);
  socket.on('data', d => { if (!entry.first) entry.first = d.toString('latin1').split('\r\n')[0].slice(0, 120); });
  socket.on('error', () => {});
  setTimeout(() => socket.destroy(), 2000);
});
await new Promise(r => tcp.listen(0, '0.0.0.0', r));
const stunUser = buf => {
  // STUN attributes after the 20-byte header; USERNAME is type 0x0006.
  for (let i = 20; i + 4 <= buf.length;) {
    const type = buf.readUInt16BE(i), length = buf.readUInt16BE(i + 2);
    if (type === 0x0006) return buf.subarray(i + 4, i + 4 + length).toString('utf8');
    i += 4 + length + ((4 - (length % 4)) % 4);
  }
  return null;
};
const udpSocket = async name => {
  const s = dgram.createSocket('udp4');
  s.on('message', (msg, from) => report.hits[name].push({ from: `${from.address}:${from.port}`, bytes: msg.length, stunType: msg.length >= 2 ? msg.readUInt16BE(0).toString(16) : '', username: stunUser(msg) }));
  await new Promise(r => s.bind(0, '0.0.0.0', r));
  return s;
};
const udp = await udpSocket('udp'), udp2 = await udpSocket('udp2');
const lan = Object.values(os.networkInterfaces()).flat().find(i => i && i.family === 'IPv4' && !i.internal)?.address ?? '127.0.0.1';

let app, browser;
const threads = pid => Number(spawnSync('powershell', ['-NoProfile', '-Command', `(Get-Process -Id ${pid}).Threads.Count`], { encoding: 'utf8' }).stdout.trim());
try {
  const running = spawnSync('tasklist', ['/FI', 'IMAGENAME eq atlas-native.exe'], { encoding: 'utf8' }).stdout;
  if (running.includes('atlas-native.exe')) throw new Error('another InfraWerf is running');
  app = spawn(path.resolve(process.env.ATLAS_DESKTOP_BINARY ?? 'target/release/atlas-native.exe'), [], {
    cwd: root, windowsHide: true, stdio: 'ignore',
    env: { ...Object.fromEntries(Object.entries(process.env).filter(([k]) => k !== 'ATLAS_LEGACY_DB' && k !== 'ATLAS_REPORTS_DIR')), ATLAS_NATIVE_DB: path.join(work, 'native.sqlite3'), ATLAS_SSH_CONFIG: path.join(work, 'ssh_config'), ATLAS_PLUGINS_DIR: plugins, WEBVIEW2_USER_DATA_FOLDER: path.join(work, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9243 --remote-debugging-address=127.0.0.1' },
  });
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:9243/json/version')).ok) break; } catch {} await delay(200); }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9243');
  const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('http://127.0.0.1')) ?? browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'View host', host: 'secret-host.home.arpa' } });
  const host = (await invoke('map_state')).servers[0];
  const find = (value, key) => { if (value && typeof value === 'object') { if (key in value) return value[key]; for (const v of Object.values(value)) { const f = find(v, key); if (f !== undefined) return f; } } return undefined; };
  const limit = find(await invoke('monitoring_state'), 'history_limit') ?? 1000;
  report.history_limit = limit;
  await writeFile(path.join(pluginDir, 'view/config.js'), `window.REVIEW = ${JSON.stringify({ tcp: tcp.address().port, udp: udp.address().port, udp2: udp2.address().port, lan, flood: limit + 50 })};\n`);
  await invoke('plugin_enable', { pluginId: ID, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: ID, serverId: host.id, capabilities: ['read_saved_state', 'ssh_session', 'run_approved_command'] });
  const log = async () => (await invoke('plugin_activity', { limit: 100000 })).entries;
  report.log_before = (await log()).map(e => [e.action, e.outcome, e.detail]);
  report.views = (await invoke('plugin_views', { serverId: host.id })).views;
  if (!report.views.length) { report.plugins_state = JSON.stringify(await invoke('plugins_state')).slice(0, 3000); throw new Error('the review view is not offered'); }

  await page.reload();
  await page.getByRole('button', { name: 'Open View host workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'View host', exact: true });
  // Fallback if the tab is not offered: open the view through the same
  // command the tab uses, over the workspace body.
  let viaTab = true;
  await workspace.getByRole('tab', { name: 'Review attempts', exact: true }).click({ timeout: 20000 }).catch(async () => {
    viaTab = false;
    await page.screenshot({ path: path.join(work, 'no-tab.png') });
    const body = await workspace.locator('.workspace-body').boundingBox();
    report.opened_label = await invoke('plugin_view_open', { serverId: host.id, pluginId: ID, contributionId: 'escape2', bounds: body });
  });
  report.opened_via_tab = viaTab;
  const areaLocator = viaTab ? workspace.getByLabel('Review attempts view', { exact: true }) : workspace.locator('.workspace-body');
  const viewPage = async predicate => { for (let i = 0; i < 150; i++) { for (const c of browser.contexts()) for (const p of c.pages()) if (predicate(p.url())) return p; await delay(200); } throw new Error('no view page'); };
  const view = await viewPage(u => u.startsWith('http://127.0.0.1:') && u.endsWith('/view/index.html'));
  const viewUrl = new URL(view.url());
  await expect.poll(() => view.title(), { timeout: 90000 }).toBe('phase1');
  await delay(3000);
  report.phase1 = JSON.parse(await view.locator('#results').innerText());

  // A local process without the secret: unbounded idle connections.
  const before = threads(app.pid);
  const sockets = [];
  for (let i = 0; i < 1000; i++) { const s = net.connect(Number(viewUrl.port), '127.0.0.1'); s.on('error', () => {}); s.write('GET / HTTP/1.1\r\n'); sockets.push(s); }
  await delay(3000);
  report.slowloris = { threads_before: before, threads_with_1000_idle_connections: threads(app.pid) };
  // Keep them alive past the 10 s read timeout by trickling a header line.
  for (let round = 0; round < 2; round++) { await delay(7000); for (const s of sockets) s.write('X-a: b\r\n'); }
  report.slowloris.threads_after_17s_trickle = threads(app.pid);
  const probe = () => view.evaluate(() => fetch(new URL('../infrawerf/v1/context', location.href)).then(r => r.status).catch(e => String(e)));
  report.slowloris.view_still_answers = await probe();
  for (const s of sockets) s.destroy();
  await delay(1000);
  report.slowloris.view_answers_after_release = await probe();
  // A local process that keeps 16 connections open, reopening each as it
  // is dropped, so the page's own requests find no free slot.
  let holding = true; const held = [];
  const hold = () => { if (!holding) return; const s = net.connect(Number(viewUrl.port), '127.0.0.1'); s.on('error', () => {}); s.on('close', () => setTimeout(hold, 10)); s.write('GET / HTTP/1.1\r\n'); held.push(s); };
  for (let i = 0; i < 40; i++) hold();
  await delay(1500);
  const samples = [];
  for (let i = 0; i < 8; i++) { samples.push(await probe()); await delay(1000); }
  report.slowloris.page_requests_while_16_slots_held = samples;
  holding = false; for (const s of held) s.destroy();
  await delay(1000);
  report.slowloris.view_answers_after_hold = await probe();

  let entries = await log();
  report.blocked_after_phase1 = entries.filter(e => e.plugin_id === ID && e.action === 'view_blocked').map(e => e.detail);
  const marker = entries[entries.length - 1];
  report.oldest_entry_before_flood = marker && [marker.action, marker.outcome, marker.detail, marker.at ?? marker.time];
  report.entries_before_flood = entries.length;

  await view.evaluate(n => { (window.GO ||= {})[n] = true; }, '2').catch(() => {});
  await expect.poll(() => view.title(), { timeout: 600000, intervals: [1000] }).toBe('phase2');
  report.phase2 = JSON.parse(await view.locator('#results').innerText());
  entries = await log();
  report.after_flood = {
    total_entries: entries.length,
    view_blocked_entries: entries.filter(e => e.plugin_id === ID && e.action === 'view_blocked').length,
    refused_view_requests: entries.filter(e => e.plugin_id === ID && e.action === 'view_request' && e.outcome === 'refused').length,
    other_entries_left: entries.filter(e => !(e.action === 'view_request' && e.outcome === 'refused')).map(e => [e.action, e.outcome, e.detail]),
    sample_detail: entries.find(e => e.action === 'view_request')?.detail,
  };

  await view.evaluate(n => { (window.GO ||= {})[n] = true; }, '3').catch(() => {});
  const ask = page.getByRole('dialog', { name: 'A plugin asks to run a command', exact: true });
  await expect(ask).toBeVisible({ timeout: 20000 });
  const shownAt = Date.now();
  await expect.poll(() => view.title(), { timeout: 20000 }).toBe('phase3');
  const phase3 = JSON.parse(await view.locator('#results').innerText());
  const run = await ask.getByRole('button', { name: 'Run', exact: true }).boundingBox();
  const area = await areaLocator.boundingBox();
  report.dialog = {
    request_to_dialog_visible_ms_upper_bound: shownAt - phase3.ask_sent_at.value,
    run_button: run, view_area: area,
    run_button_inside_view_area: run.x >= area.x && run.y >= area.y && run.x + run.width <= area.x + area.width && run.y + run.height <= area.y + area.height,
    run_enabled_immediately: await ask.getByRole('button', { name: 'Run', exact: true }).isEnabled(),
    focused: await page.evaluate(() => document.activeElement?.textContent),
  };
  await expect(ask.getByRole('button', { name: 'Run', exact: true })).toBeEnabled({ timeout: 5000 });
  report.dialog.run_enabled_after_ms = Date.now() - shownAt;
  await page.screenshot({ path: path.join(work, 'dialog.png') });
  await ask.getByRole('button', { name: 'Decline', exact: true }).click();

  await view.evaluate(n => { (window.GO ||= {})[n] = true; }, '4').catch(() => {});
  let blob;
  for (let i = 0; i < 100 && !blob; i++) { for (const c of browser.contexts()) for (const p of c.pages()) { const t = await p.title().catch(() => ''); if (t.startsWith('blob:')) blob = { url: p.url().slice(0, 60), title: t }; } if (!blob) await delay(200); }
  report.blob = blob ?? 'no blob page seen';
  entries = await log();
  report.blocked_after_blob = entries.filter(e => e.plugin_id === ID && e.action === 'view_blocked').slice(-5).map(e => e.detail);
  report.map_after = (await invoke('map_state')).servers.map(s => s.host);

  if (viaTab) await workspace.getByRole('button', { name: 'Lock view', exact: true }).click(); else await invoke('plugin_view_close', { label: report.opened_label });
  await delay(1000);
  report.views_open_after_lock = await invoke('plugin_views_open', {});
  report.port_after_lock = await new Promise(r => { const s = net.connect(Number(viewUrl.port), '127.0.0.1'); s.on('connect', () => { s.destroy(); r('open'); }); s.on('error', () => r('closed')); });
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode, { timeout: 20000 }).not.toBe(null);
  // Read the log once InfraWerf has closed the database.
  const { DatabaseSync } = await import('node:sqlite').catch(() => ({}));
  if (DatabaseSync) {
    const db = new DatabaseSync(path.join(work, 'native.sqlite3'), { readOnly: true });
    report.db_after_close = { rows: db.prepare('select count(*) n from plugin_activity').get().n, by_kind: db.prepare('select action, outcome, count(*) n from plugin_activity group by 1,2').all(), view_blocked_tail: db.prepare("select detail from plugin_activity where action='view_blocked' order by id desc limit 4").all().map(r => r.detail) };
    db.close();
  }
} finally {
  await writeFile(path.join(work, 'review-result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(r => app.once('exit', r)); }
  tcp.close(); udp.close(); udp2.close();
}
