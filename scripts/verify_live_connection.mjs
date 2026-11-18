// Explicit owner-authorized read/session check. Writes only a synthetic local DB.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import net from 'node:net';
import path from 'node:path';

const host = process.env.ATLAS_LIVE_TEST_HOST;
// The owner lists the hosts they approve for live checks (OpenSSH aliases,
// comma-separated) in ATLAS_LIVE_APPROVED_HOSTS; nothing else is contacted.
const approved = (process.env.ATLAS_LIVE_APPROVED_HOSTS ?? '').split(',').map(name => name.trim()).filter(Boolean);
if (!host || !approved.includes(host)) throw new Error('Select a host the owner listed in ATLAS_LIVE_APPROVED_HOSTS.');
if (!process.env.ATLAS_DESKTOP_BINARY) throw new Error('Select the actual desktop executable.');
const directory = path.resolve('.tools', `live-connection-proof-${randomUUID()}`);
await mkdir(directory);
const listener = net.createServer();
await new Promise(resolve => listener.listen(0, '127.0.0.1', resolve));
const port = listener.address().port;
await new Promise(resolve => listener.close(resolve));
const { ATLAS_NATIVE_DB, ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ATLAS_SSH_CONFIG, ...environment } = process.env;
const app = spawn(path.resolve(process.env.ATLAS_DESKTOP_BINARY), [], { windowsHide: true, stdio: 'ignore', env: {
  ...environment, ATLAS_NATIVE_DB: path.join(directory, 'synthetic.sqlite3'),
  WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'),
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
} });
const endpoint = `http://127.0.0.1:${port}`;
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const results = [];
let browser, page, session;
try {
  for (let i = 0; i < 150; i++) {
    try { if ((await fetch(`${endpoint}/json/version`)).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP(endpoint);
  page = browser.contexts()[0].pages()[0];
  await page.waitForLoadState('domcontentloaded');
  await page.locator('.app-shell').waitFor();
  const invoke = (command, args) => page.evaluate(async ({ command, args }) => {
    try { return { ok: true, value: await window.__TAURI_INTERNALS__.invoke(command, args) }; }
    catch (error) { return { ok: false, code: error?.code ?? 'unstructured', message: error?.message ?? String(error) }; }
  }, { command, args });
  const map = await invoke('add_server', { input: { name: 'Approved connection diagnostic', host } });
  if (!map.ok) throw new Error('Synthetic test store could not be prepared.');
  const serverId = map.value.servers[0].id;
  const prepared = await invoke('ssh_prepare', { serverId });
  const verified = prepared.ok ? await invoke('ssh_verify', { serverId, operationId: prepared.value.operation_id }) : prepared;
  results.push({ check: 'strict SSH verification', ok: verified.ok && verified.value.state === 'verified', state: verified.value?.state, code: verified.code });
  const refresh = await invoke('inventory_refresh', { serverId });
  results.push({ check: 'inventory refresh', ok: refresh.ok && ['updated', 'unchanged'].includes(refresh.value?.status), state: refresh.value?.status, code: refresh.code, sourceTime: refresh.value?.source_collected_at, receivedTime: refresh.value?.refreshed_at });
  const ticket = await invoke('files_prepare', { serverId });
  const opened = ticket.ok ? await invoke('files_open', { serverId, operationId: ticket.value.operation_id }) : ticket;
  results.push({ check: 'protected SFTP session', ok: opened.ok, code: opened.code });
  if (opened.ok) {
    session = opened.value.session_id;
    const terminal = await invoke('files_terminal_start', { serverId, sessionId: session, cols: 80, rows: 24 });
    results.push({ check: 'interactive terminal connect', ok: terminal.ok, code: terminal.code });
    await invoke('files_lock', { sessionId: session }); session = null;
  }
  const report = { host, realHostUsed: true, remoteWritesPerformed: false, ownerDatabaseUsed: false, results };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ directory, ...report }, null, 2));
  if (results.some(item => !item.ok)) process.exitCode = 1;
} finally {
  if (page && !page.isClosed()) {
    if (session) await page.evaluate(id => window.__TAURI_INTERNALS__.invoke('files_lock', { sessionId: id }), session).catch(() => {});
    await page.getByRole('button', { name: 'Quit InfraWerf', exact: true }).click({ timeout: 5000 }).catch(() => {});
  }
  for (let i = 0; i < 50; i++) {
    try { await fetch(`${endpoint}/json/version`); } catch { break; }
    await delay(100);
  }
  await browser?.close().catch(() => {});
  if (app.exitCode === null) app.kill();
}
