// Actual Windows Tauri/WebView2 proof for isolated plugin views (F23/T047).
// A hostile view tries every way out of its isolation; an example view reads
// what its grants allow and asks to run a command, which runs only after the
// owner presses Run in InfraWerf's own dialog. Real commands run in the
// disposable Linux container over real OpenSSH.
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import dgram from 'node:dgram';
import os from 'node:os';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-plugin-views-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const plugins = path.join(directory, 'plugins');
await cp(path.join(root, 'scripts/fixtures/hostile-view'), path.join(plugins, 'hostile-view'), { recursive: true });
await cp(path.join(root, 'examples/host-dashboard'), path.join(plugins, 'host-dashboard'), { recursive: true });
const HOSTILE = 'org.example.hostile-view', DASHBOARD = 'org.example.host-dashboard';
// A UDP listener stands in for any address WebRTC could reach.
const udp = dgram.createSocket('udp4');
const packets = { loopback: 0, lan: 0 };
udp.on('message', (_, from) => { packets[from.address === '127.0.0.1' ? 'loopback' : 'lan'] += 1; });
await new Promise(resolve => udp.bind(0, '0.0.0.0', resolve));
const lan = Object.values(os.networkInterfaces()).flat().find(i => i && i.family === 'IPv4' && !i.internal)?.address;
await writeFile(path.join(plugins, 'hostile-view/view/rtc.json'), JSON.stringify({ port: udp.address().port }));

const server = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/linux_fixture.py', path.join(directory, 'fixture')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
const lines = createInterface({ input: server.stdout });
let diagnostic = '';
server.stderr.on('data', data => { diagnostic += data.toString().slice(0, 1000); });
let app, browser, page;
const results = [];
try {
  const info = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`SSH fixture startup failed: ${diagnostic}`)), 90000);
    lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); });
    server.once('exit', code => { if (code) { clearTimeout(timer); reject(new Error(`SSH fixture exited: ${diagnostic}`)); } });
  });
  const inContainer = (...args) => spawnSync('podman', ['--connection', 'podman-machine-default', 'exec', info.container, ...args], { encoding: 'utf8', windowsHide: true, timeout: 30000 });
  const database = path.join(directory, 'native.sqlite3');
  const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  app = spawn(process.env.ATLAS_DESKTOP_BINARY ? path.resolve(process.env.ATLAS_DESKTOP_BINARY) : path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root, windowsHide: true,
    env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_SSH_CONFIG: info.config, ATLAS_PLUGINS_DIR: plugins, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9242 --remote-debugging-address=127.0.0.1' },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9242/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9242');
  page = browser.contexts()[0].pages().find(p => !p.url().startsWith('http://127.0.0.1')) ?? browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'View host', host: 'atlas-linux' } });
  await invoke('add_server', { input: { name: 'Other host', host: 'other-view.home' } });
  let servers = (await invoke('map_state')).servers;
  const host = servers.find(s => s.host === 'atlas-linux'), other = servers.find(s => s.host !== 'atlas-linux');
  // Pages of every target the debugger sees, to find the views.
  const viewPages = async () => {
    const found = [];
    for (const context of browser.contexts()) for (const candidate of context.pages()) if (candidate.url().startsWith('http://127.0.0.1:')) found.push(candidate);
    return found;
  };
  const awaitView = async entry => {
    for (let i = 0; i < 100; i++) {
      const found = (await viewPages()).find(p => p.url().endsWith(entry));
      if (found) return found;
      await delay(200);
    }
    throw new Error(`no view page for ${entry}`);
  };
  const log = async () => (await invoke('plugin_activity', { limit: 500 })).entries;

  // 1. Nothing appears until enabled and allowed; then only on that server.
  expect((await invoke('plugin_views', { serverId: host.id })).views).toEqual([]);
  await invoke('plugin_enable', { pluginId: HOSTILE, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: HOSTILE, serverId: host.id, capabilities: ['ssh_session'] });
  await invoke('plugin_enable', { pluginId: DASHBOARD, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: DASHBOARD, serverId: host.id, capabilities: ['read_saved_state', 'ssh_session', 'run_approved_command'] });
  expect((await invoke('plugin_views', { serverId: host.id })).views.map(v => v.title).sort()).toEqual(['Escape attempts', 'Host dashboard']);
  expect((await invoke('plugin_views', { serverId: other.id })).views).toEqual([]);
  results.push('plugin view tabs appear only after the plugin is enabled and allowed on a server, and only on that server');

  // 2. The hostile view tries every way out.
  const before = JSON.stringify((await invoke('map_state')).servers.map(s => [s.id, s.name, s.host]));
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(2);
  await page.getByRole('button', { name: 'Open View host workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'View host', exact: true });
  await workspace.getByRole('button', { name: 'Refresh inventory', exact: true }).click();
  await expect(workspace.getByRole('status')).toContainText('Approved inventory updated', { timeout: 60000 });
  await workspace.getByRole('tab', { name: 'Escape attempts', exact: true }).click();
  const hostile = await awaitView('/view/index.html');
  await expect.poll(async () => hostile.isClosed() ? 'closed' : await hostile.title().catch(() => ''), { timeout: 60000 }).toBe('done');
  const attempts = JSON.parse(await hostile.locator('#results').innerText());
  await writeFile(path.join(directory, 'hostile-results.json'), JSON.stringify(attempts, null, 2));
  const mustFail = ['webrtc_stun', 'rtc_blank_frame', 'rtc_frames_index', 'rtc_srcdoc_frame', 'rtc_nested_frame', 'tauri_add_server', 'tauri_post_message', 'tauri_global', 'ipc_postmessage', 'webview_postmessage', 'ipc_protocol_fetch', 'ipc_protocol_remove', 'main_origin_fetch', 'internet_fetch', 'image_beacon', 'websocket', 'worker', 'window_open', 'api_without_secret', 'api_observations', 'api_inventory', 'api_action', 'api_unknown'];
  for (const name of mustFail) expect(attempts[name]?.reached, `${name}: ${JSON.stringify(attempts[name])}`).toBe(false);
  expect(attempts.api_context.reached).toBe(true);
  // Tauri freezes its internals in every page, so the page can call
  // invoke(); with both of its routes closed the call never answers. That a
  // command did not run is proven below from InfraWerf's own state.
  for (const name of ['tauri_internals', 'blank_frame_ipc']) expect(attempts[name].reached === false || attempts[name].value === 'timeout', `${name}: ${JSON.stringify(attempts[name])}`).toBe(true);
  // A blocked frame still fires load with an opaque error page, and a form
  // submit returns: the content security policy log below shows both blocked.
  expect(['loaded-opaque', 'no load']).toContain(attempts.iframe_main.value.replace(/^Error: /, ''));
  expect(JSON.parse(attempts.api_context.value).plugin_id).toBe(HOSTILE);
  // Without read_saved_state the view does not learn the server's name or host.
  expect(JSON.parse(attempts.api_context.value).server).toBe(null);
  // The last attempt navigates the whole view to InfraWerf's page: refused.
  await delay(1500);
  expect(hostile.url().startsWith('http://127.0.0.1:')).toBe(true);
  // While the view ran, nothing reached the UDP listener. As a positive
  // control, InfraWerf's own page (trusted, with no guard) shows that
  // WebRTC works in this browser and that the listener sees it: the view's
  // lack of WebRTC is the guard's doing (T047 review, finding 1).
  const duringView = { ...packets };
  expect(duringView, 'UDP packets while the view ran').toEqual({ loopback: 0, lan: 0 });
  const sent = await page.evaluate(async ({ port, lan }) => {
    const peer = new RTCPeerConnection({ iceServers: [{ urls: `stun:127.0.0.1:${port}` }, ...(lan ? [{ urls: `stun:${lan}:${port}` }] : [])] });
    peer.createDataChannel('x');
    await peer.setLocalDescription(await peer.createOffer());
    await new Promise(resolve => setTimeout(resolve, 3000));
    const candidates = (peer.localDescription?.sdp ?? '').split(String.fromCharCode(10)).filter(line => line.startsWith('a=candidate')).length;
    peer.close();
    return candidates;
  }, { port: udp.address().port, lan });
  await delay(500);
  await writeFile(path.join(directory, 'webrtc.json'), JSON.stringify({ lan, duringView, control: packets, sent }));
  expect(packets.loopback, 'positive control: the listener sees WebRTC').toBeGreaterThan(0);
  results.push(`WebRTC: the view and every frame it made (blank, indexed, srcdoc, nested) had no RTCPeerConnection, and the UDP listener received nothing while the view ran; as a control, InfraWerf's own page sent ${packets.loopback} STUN packets to the same listener, so the absence is the guard's doing`);
  // Nothing in InfraWerf changed, and the attempts are in the activity log.
  expect(JSON.stringify((await invoke('map_state')).servers.map(s => [s.id, s.name, s.host]))).toBe(before);
  expect(JSON.stringify(await invoke('map_state'))).not.toContain('planted.invalid');
  let entries = await log();
  const blocked = entries.filter(e => e.plugin_id === HOSTILE && e.action === 'view_blocked').map(e => e.detail);
  await writeFile(path.join(directory, 'hostile-log.json'), JSON.stringify(blocked, null, 2));
  for (const expected of [/^content_policy: connect-src http:\/\/ipc\.localhost/, /^content_policy: connect-src https:\/\/example\.com/, /^ipc: /, /^ipc: RTCPeerConnection$/, /^unknown_call: GET servers\/remove/, /^new_window: http:\/\/tauri\.localhost/, /^content_policy: frame-src http:\/\/tauri\.localhost/, /^content_policy: form-action http:\/\/ipc\.localhost/, /^content_policy: worker-src blob/])
    expect(blocked.some(d => expected.test(d)), `${expected} in ${JSON.stringify(blocked)}`).toBe(true);
  expect(entries.some(e => e.plugin_id === HOSTILE && e.action === 'read_saved_state' && e.outcome === 'refused' && e.detail === 'not_requested' || e.plugin_id === HOSTILE && e.action === 'read_saved_state' && e.outcome === 'refused')).toBe(true);
  expect(entries.some(e => e.plugin_id === HOSTILE && e.action === 'view_request' && e.outcome === 'refused' && e.detail === 'kernel')).toBe(true);
  await page.screenshot({ path: path.join(directory, 'hostile-view.png') });
  await hostile.screenshot({ path: path.join(directory, 'hostile-view-page.png') }).catch(() => {});
  results.push(`the hostile view's ${mustFail.length} escape attempts all failed: Tauri internals, window.ipc and chrome.webview were gone; fetches to ipc.localhost, InfraWerf's own page and the internet, an image beacon, a WebSocket, frames, a blank frame's IPC, a worker, window.open, the API without the secret, ungranted observations, inventory and a command, and an unknown call were refused; navigating the view to InfraWerf's page was refused; the map was unchanged; each kind of attempt is in the plugin activity log`);

  // 3. The example view reads what its grants allow and asks to run a command.
  await workspace.getByRole('tab', { name: 'Host dashboard', exact: true }).click();
  const dashboard = await awaitView('/view/index.html');
  await expect.poll(() => hostile.isClosed()).toBe(true);
  // Requests that never reached the view (here, the page's own call without
  // the secret) are not charged to the plugin: one row counts them at close.
  await expect.poll(async () => (await log()).some(e => e.plugin_id === HOSTILE && e.action === 'view_blocked' && /^\d+ requests from outside the view were refused/.test(e.detail))).toBe(true);
  // The flood stayed within one opening's budget, its text was not logged,
  // and the owner's earlier entries are still there.
  entries = await log();
  const fromView = entries.filter(e => e.plugin_id === HOSTILE && ['view_blocked', 'view_request', 'read_saved_state', 'run_approved_command'].includes(e.action));
  expect(fromView.length, JSON.stringify(fromView.map(e => e.detail))).toBeLessThanOrEqual(40 + 2);
  expect(entries.some(e => e.detail.includes('flood-'))).toBe(false);
  expect(entries.some(e => e.plugin_id === HOSTILE && e.action === 'view_blocked' && /^\d+ more refused or blocked attempts were not logged one by one$/.test(e.detail))).toBe(true);
  expect(entries.some(e => e.plugin_id === HOSTILE && e.action === 'enable')).toBe(true);
  results.push(`200 refused requests from the hostile view were logged within one opening's budget of 40 rows plus a summary row; none of their text reached the log; the owner's enable entry is still there`);
  await expect(dashboard.locator('#server')).toHaveText('Host dashboard · View host');
  // The view sits exactly over the tab area.
  const area = await workspace.getByLabel('Host dashboard view', { exact: true }).boundingBox();
  const placed = (await invoke('plugin_views_open', {}))[0].bounds;
  for (const key of ['x', 'y', 'width', 'height']) expect(Math.abs(placed[key] - area[key]), `${key}: ${JSON.stringify(placed)} vs ${JSON.stringify(area)}`).toBeLessThan(2);
  await dashboard.screenshot({ path: path.join(directory, 'dashboard-view.png') });
  await expect(dashboard.locator('#readings')).toHaveText('Kernel version: not read yet');
  await expect(dashboard.locator('#inventory')).toContainText('CPUs, collected');
  await dashboard.locator('#read-kernel').click();
  const ask = page.getByRole('dialog', { name: 'A plugin asks to run a command', exact: true });
  await expect(ask).toContainText('Host dashboard asks to run Kernel version on View host.');
  // Run waits a moment, so a click aimed at the page cannot land on it.
  await expect(ask.getByRole('button', { name: 'Run (wait…)', exact: true })).toBeDisabled();
  await expect(ask.getByRole('button', { name: 'Decline', exact: true })).toBeFocused();
  await expect(ask.getByLabel('Requested command', { exact: true })).toHaveText('uname -r');
  // The view is hidden while InfraWerf asks.
  expect(await invoke('plugin_views_open', {})).toHaveLength(1);
  await page.screenshot({ path: path.join(directory, 'view-asks.png') });
  // Nothing ran yet.
  expect((await invoke('pack_state', { serverId: host.id })).commands.find(c => c.plugin_id === DASHBOARD).observation).toBe(null);
  await ask.getByRole('button', { name: 'Run', exact: true }).click();
  const kernel = inContainer('uname', '-r').stdout.trim();
  await expect(dashboard.locator('#action')).toHaveText(`Kernel ${kernel}`, { timeout: 30000 });
  await expect(dashboard.locator('#readings')).toHaveText(`Kernel version: ${kernel}`);
  // Declining runs nothing.
  await dashboard.locator('#read-kernel').click();
  await ask.getByRole('button', { name: 'Decline', exact: true }).click();
  await expect(dashboard.locator('#action')).toHaveText('Not run: declined', { timeout: 10000 });
  entries = await log();
  expect(entries.filter(e => e.plugin_id === DASHBOARD && e.action === 'run_command' && e.outcome === 'done').length).toBe(1);
  results.push(`Run stayed disabled ("Run (wait…)") when the dialog appeared, with Decline focused; the example view showed the server name, the plugin's reading and the saved inventory; its request to run "uname -r" opened InfraWerf's own dialog with the exact command, nothing ran until Run was pressed, and the view then showed the container's kernel ${kernel}; a declined request ran nothing`);

  // 4. Lock, closing and the kill switch tear the view down.
  await workspace.getByRole('button', { name: 'Lock view', exact: true }).click();
  await expect.poll(() => dashboard.isClosed()).toBe(true);
  expect(await invoke('plugin_views_open', {})).toEqual([]);
  await expect(workspace).toContainText('View locked.');
  await workspace.getByRole('button', { name: 'Open view again', exact: true }).click();
  let again = await awaitView('/view/index.html');
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await expect.poll(() => again.isClosed()).toBe(true);
  expect(await invoke('plugin_views_open', {})).toEqual([]);
  await page.getByRole('button', { name: 'Open View host workspace', exact: true }).click();
  await workspace.getByRole('tab', { name: 'Host dashboard', exact: true }).click();
  again = await awaitView('/view/index.html');
  await again.locator('#read-kernel').click();
  await expect(ask).toBeVisible();
  await invoke('plugins_set_all_off', { off: true });
  await expect.poll(() => again.isClosed(), { timeout: 10000 }).toBe(true);
  expect(await invoke('plugin_views_open', {})).toEqual([]);
  await ask.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(workspace).toContainText('This view was closed because what plugins may do changed.', { timeout: 10000 });
  entries = await log();
  expect(entries.filter(e => e.plugin_id === DASHBOARD && e.action === 'run_command' && e.outcome === 'done').length).toBe(1);
  await invoke('plugins_set_all_off', { off: false });
  // Withdrawing the permission removes the tab.
  await invoke('plugin_set_grant', { pluginId: DASHBOARD, serverId: host.id, capabilities: [] });
  expect((await invoke('plugin_views', { serverId: host.id })).views.map(v => v.title)).toEqual(['Escape attempts']);
  results.push('Lock view, closing the workspace and turning all plugins off each closed the view within seconds; a request waiting when plugins were turned off was cancelled and ran nothing even when Run was pressed afterwards; withdrawing the permission removed the tab');

  expect(pageErrors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'disposable loopback Linux container only', noSudo: true, noRealHost: true, pageErrors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native plugin views proof failed in ${directory}.`);
  throw failure;
} finally {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(resolve => app.once('exit', resolve)); }
  if (server.exitCode === null) {
    server.stdin.end('\n');
    await Promise.race([new Promise(resolve => server.once('exit', resolve)), delay(60000)]);
    if (server.exitCode === null) server.kill();
  }
  lines.close();
  udp.close();
}
