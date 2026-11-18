// Actual Windows Tauri/WebView2 proof for pack additions to existing screens
// (F21/T044): an application in the Apps list through the existing tunnel,
// badges from pack observations, relationship kinds and a workload kind. A
// disposable Linux container over real OpenSSH is the only target.
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { copyFile, mkdir, writeFile } from 'node:fs/promises';
import net from 'node:net';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-pack-map-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const listening = port => new Promise(resolve => { const s = net.connect({ port, host: '127.0.0.1', timeout: 1500 }); s.once('connect', () => { s.destroy(); resolve(true); }); s.once('error', () => resolve(false)); s.once('timeout', () => { s.destroy(); resolve(false); }); });
const plugins = path.join(directory, 'plugins');
await mkdir(path.join(plugins, 'homelab-map'), { recursive: true });
await copyFile(path.join(root, 'examples/homelab-map/infrawerf-plugin.json'), path.join(plugins, 'homelab-map/infrawerf-plugin.json'));
const CAPS = ['ssh_session', 'run_approved_command'];
// The container has no systemd, so a second pack reads what it does have.
const fixturePack = {
  id: 'org.example.map-fixture', name: 'Map fixture', version: '1.0.0', api_version: 2, capabilities: ['read_saved_state', ...CAPS],
  contributions: [
    { id: 'processes', point: 'collector', title: 'Processes', capabilities: CAPS, command: { argv: ['ps', '-e', '-o', 'comm=,stat='], extract: { kind: 'table', columns: ['command', 'state'] } }, workload_kind: { name_column: 'command', state_column: 'state' } },
    { id: 'marker', point: 'health_check', title: 'Marker', capabilities: CAPS, command: { argv: ['basename', '/srv/atlas-ok'], extract: { kind: 'line' }, healthy: ['atlas-ok'] } },
    { id: 'marker-badge', point: 'server_badge', title: 'Marker', capabilities: ['read_saved_state'], badge: { observation: 'marker' } },
  ],
};
await mkdir(path.join(plugins, 'map-fixture'), { recursive: true });
await writeFile(path.join(plugins, 'map-fixture/infrawerf-plugin.json'), JSON.stringify(fixturePack, null, 2));
// An application on another machine is never listed as a plugin.
await mkdir(path.join(plugins, 'elsewhere'), { recursive: true });
await writeFile(path.join(plugins, 'elsewhere/infrawerf-plugin.json'), JSON.stringify({ id: 'org.example.elsewhere', name: 'Elsewhere', version: '1.0.0', api_version: 2, capabilities: ['ssh_session', 'loopback_tunnel'], contributions: [{ id: 'nas', point: 'remote_app', title: 'NAS', capabilities: ['ssh_session', 'loopback_tunnel'], app: { port: 443, host: '192.168.1.10' } }] }));

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
  // The application the example pack names: a page on the container's own localhost only.
  const marker = `atlas-status-${randomUUID()}`;
  const started = spawnSync('podman', ['--connection', 'podman-machine-default', 'exec', '-d', info.container, 'node', '-e', `require('http').createServer((q,s)=>{s.setHeader('content-type','text/html');s.end('<title>Status page</title><p>${marker} '+q.url+'</p>')}).listen(8099,'127.0.0.1')`], { encoding: 'utf8', windowsHide: true, timeout: 30000 });
  expect(started.status).toBe(0);
  await expect.poll(() => inContainer('node', '-e', "require('http').get('http://127.0.0.1:8099/status',r=>process.exit(r.statusCode===200?0:1)).on('error',()=>process.exit(1))").status, { timeout: 15000 }).toBe(0);

  const database = path.join(directory, 'native.sqlite3');
  const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  app = spawn(process.env.ATLAS_DESKTOP_BINARY ? path.resolve(process.env.ATLAS_DESKTOP_BINARY) : path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root, windowsHide: true,
    env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_SSH_CONFIG: info.config, ATLAS_PLUGINS_DIR: plugins, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9239 --remote-debugging-address=127.0.0.1' },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9239/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9239');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  const rejected = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args).then(() => null, e => e?.code ?? String(e)), { name, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();

  await invoke('add_server', { input: { name: 'Map fixture host', host: 'atlas-linux' } });
  await invoke('add_server', { input: { name: 'Backup target', host: 'backup-fixture.home' } });
  await invoke('add_server', { input: { name: 'Third host', host: 'third-fixture.home' } });
  const servers = (await invoke('map_state')).servers;
  const host = servers.find(s => s.host === 'atlas-linux'), backup = servers.find(s => s.host === 'backup-fixture.home'), third = servers.find(s => s.host === 'third-fixture.home');
  const MAP = 'org.example.homelab-map', FIXTURE = 'org.example.map-fixture';
  const ALL = ['read_saved_state', 'ssh_session', 'run_approved_command', 'loopback_tunnel'];
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(3);
  const nav = page.getByRole('navigation', { name: 'Main navigation' });
  const cardOf = s => page.locator(`.server-card[data-server-id="${s.id}"]`);
  const openApp = { serverId: host.id, pluginId: MAP, contributionId: 'status-page' };

  // 1. Found packs add nothing; a pack naming another machine is unusable.
  let map = await invoke('pack_map');
  expect(map.badges).toEqual([]); expect(map.relationship_kinds).toEqual([]);
  expect((await invoke('pack_apps', { serverId: host.id })).apps).toEqual([]);
  expect(await rejected('pack_app_open', openApp)).toBe('denied');
  const state = await invoke('plugins_state');
  expect(state.plugins.map(p => p.id).sort()).toEqual([MAP, FIXTURE]);
  expect(state.invalid.map(i => i.folder)).toEqual(['elsewhere']);
  expect(state.invalid[0].problems.join(' ')).toContain('invalid_app');
  // The owner reads where the application is before enabling.
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('region', { name: 'Plugins', exact: true }).getByRole('button', { name: 'Configure Homelab map', exact: true }).click();
  const pluginPage = page.getByRole('region', { name: 'Homelab map plugin', exact: true });
  await expect(pluginPage.getByLabel('Status page address', { exact: true })).toHaveText('localhost:8099/status');
  await expect(pluginPage).toContainText('It cannot reach another machine.');
  await expect(pluginPage).toContainText('Shows the latest result of “System state” on each server card. It never runs the command itself.');
  await expect(pluginPage).toContainText('A dashed blue line you can choose when documenting a relationship. It applies nothing.');
  await expect(pluginPage).toContainText('Shows each row as a workload named by its unit column, with its active as the state.');
  await page.screenshot({ path: path.join(directory, 'pack-map-review.png') });
  results.push('found packs add no badge, kind or application and an open is refused; a pack whose application names another machine is listed as unusable with invalid_app; the plugin page shows the application\'s localhost address, the badge\'s source, the line style and the workload kind before enabling');

  // 2. Read-only: badge and kind appear where allowed, the application does not, because a tunnel is a change.
  await pluginPage.getByRole('radio', { name: /^Read-only/ }).check();
  await pluginPage.getByRole('button', { name: 'Enable plugin', exact: true }).click();
  await expect(pluginPage).toContainText('Status: Enabled · Read-only');
  await invoke('plugin_set_grant', { pluginId: MAP, serverId: host.id, capabilities: ALL });
  map = await invoke('pack_map');
  expect(map.badges.map(b => [b.server_id, b.title, b.state, b.value])).toEqual([[host.id, 'System state', 'unknown', null]]);
  expect(map.relationship_kinds.map(k => [k.title, k.line, k.color, k.server_ids])).toEqual([['Backs up to', 'dashed', 'blue', [host.id]]]);
  expect((await invoke('pack_apps', { serverId: host.id })).apps).toEqual([]);
  expect(await rejected('pack_app_open', openApp)).toBe('denied');
  await nav.getByRole('button', { name: 'Topology', exact: true }).click();
  await expect(cardOf(host).locator('.pack-badge')).toHaveText('System state: Unknown', { timeout: 10000 });
  await expect(cardOf(backup).locator('.pack-badge')).toHaveCount(0);
  results.push('enabled read-only and allowed on one server, the badge appears on that card only, as Unknown before anything was read; the relationship kind is offered for that server; the application is not listed and opening it is refused');

  // 3. Confirm each change: the application is in the Apps list and opens through the existing tunnel.
  await invoke('plugin_disable', { pluginId: MAP });
  await invoke('plugin_enable', { pluginId: MAP, trust: 'confirm_changes' });
  expect((await invoke('pack_apps', { serverId: host.id })).apps).toEqual([{ plugin_id: MAP, plugin_name: 'Homelab map', contribution_id: 'status-page', title: 'Status page', port: 8099, path: '/status' }]);
  expect((await invoke('pack_apps', { serverId: backup.id })).apps).toEqual([]);
  await page.getByRole('button', { name: 'Open Map fixture host workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Map fixture host', exact: true });
  await workspace.getByRole('tab', { name: 'Apps', exact: true }).click();
  const fromPlugins = workspace.getByLabel('Applications from plugins', { exact: true });
  await expect(fromPlugins).toContainText('localhost:8099/status · from Homelab map');
  await fromPlugins.getByRole('button', { name: 'Open Status page', exact: true }).click();
  await expect(workspace.getByRole('status')).toContainText('Status page opened in its own window through a private tunnel.', { timeout: 30000 });
  let tunnels = await invoke('tunnel_list');
  expect(tunnels.length).toBe(1);
  let tunnel = tunnels[0];
  expect(tunnel.remote_port).toBe(8099);
  expect(tunnel.url).toBe(`http://127.0.0.1:${tunnel.local_port}/status`);
  const body = await (await fetch(tunnel.url)).text();
  expect(body).toContain(`${marker} /status`);
  results.push(`at "confirm each change" the application is listed only on the allowed server; Open started the existing loopback tunnel (127.0.0.1:${tunnel.local_port} → localhost:8099) and the page served on the container's own localhost answered through it at /status`);

  // 4. Changing what the plugin may do closes its tunnel.
  await invoke('plugin_set_grant', { pluginId: MAP, serverId: host.id, capabilities: ['read_saved_state', 'ssh_session', 'run_approved_command'] });
  await expect.poll(async () => (await invoke('tunnel_list')).length, { timeout: 10000 }).toBe(0);
  await expect.poll(() => listening(tunnel.local_port), { timeout: 10000 }).toBe(false);
  expect(await rejected('pack_app_open', openApp)).toBe('denied');
  await invoke('plugin_set_grant', { pluginId: MAP, serverId: host.id, capabilities: ALL });
  tunnel = await invoke('pack_app_open', openApp);
  expect(await listening(tunnel.local_port)).toBe(true);
  await invoke('plugins_set_all_off', { off: true });
  await expect.poll(async () => (await invoke('tunnel_list')).length, { timeout: 10000 }).toBe(0);
  await expect.poll(() => listening(tunnel.local_port), { timeout: 10000 }).toBe(false);
  await invoke('plugins_set_all_off', { off: false });
  results.push('withdrawing the tunnel permission, and turning all plugins off, each closed the pack application\'s tunnel within seconds and refused the next open');

  // 5. A badge reflects a real pack observation, and is unknown again once forgotten.
  await invoke('plugin_enable', { pluginId: FIXTURE, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: FIXTURE, serverId: host.id, capabilities: ['read_saved_state', ...CAPS] });
  await workspace.getByRole('tab', { name: 'Plugin data', exact: true }).click();
  const data = workspace.getByRole('region', { name: 'Plugin data', exact: true });
  const card = title => data.getByRole('article', { name: title, exact: true });
  await card('Marker').getByRole('button', { name: 'Run Marker', exact: true }).click();
  await expect(card('Marker').locator('.pack-value')).toHaveText('Healthy: atlas-ok', { timeout: 30000 });
  await card('System state').getByRole('button', { name: 'Run System state', exact: true }).click();
  await expect(card('System state').locator('.pack-value')).toContainText('Unknown', { timeout: 30000 });
  // The workload kind: each row of the real process table is a workload.
  await expect(card('Processes')).toContainText('Workload kind');
  await card('Processes').getByRole('button', { name: 'Run Processes', exact: true }).click();
  const kind = card('Processes').getByRole('list', { name: 'Processes workloads' });
  await expect(kind).toContainText('sshd', { timeout: 30000 });
  const listed = await kind.locator('li').count();
  expect(listed).toBeGreaterThan(2);
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await expect(cardOf(host).locator('.pack-badge.healthy')).toHaveText('Marker: Healthy · atlas-ok', { timeout: 10000 });
  await expect(cardOf(host).locator('.pack-badge.unknown')).toHaveText('System state: Unknown');
  map = await invoke('pack_map');
  const markerBadge = map.badges.find(b => b.contribution_id === 'marker-badge');
  expect(markerBadge.state).toBe('healthy');
  expect(Math.abs(Date.now() - Date.parse(markerBadge.observed_at))).toBeLessThan(120000);
  await page.screenshot({ path: path.join(directory, 'pack-badges.png') });
  await invoke('plugin_set_grant', { pluginId: FIXTURE, serverId: backup.id, capabilities: ['read_saved_state'] });
  await expect(cardOf(host).locator('.pack-badge.unknown')).toHaveCount(2, { timeout: 10000 });
  // A badge needs its command allowed too, so the backup card has none.
  await expect(cardOf(backup).locator('.pack-badge')).toHaveCount(0);
  results.push('after running the marker check, its badge on the server card showed Healthy with the value the container returned; the example\'s System state badge showed Unknown because the container has no systemd; a permission change forgot the observation and the badge went back to Unknown; a badge whose command is not allowed is not drawn; the collector\'s real process table appeared as workloads of its kind (including sshd)');

  // 6. A relationship kind can be chosen when documenting a relationship.
  await invoke('plugin_set_grant', { pluginId: MAP, serverId: backup.id, capabilities: ['read_saved_state'] });
  await cardOf(host).click();
  await page.getByRole('button', { name: '＋ Document relationship' }).first().click();
  const dialog = page.getByRole('dialog', { name: 'Document a relationship', exact: true });
  await dialog.getByLabel('From', { exact: true }).selectOption({ label: 'Map fixture host' });
  await dialog.getByLabel('To', { exact: true }).selectOption({ label: 'Third host' });
  await expect(dialog.getByLabel('Relationship kind', { exact: true })).toHaveCount(0);
  await dialog.getByLabel('To', { exact: true }).selectOption({ label: 'Backup target' });
  const kindPicker = dialog.getByLabel('Relationship kind', { exact: true });
  await expect(kindPicker.locator('option')).toHaveText(['Plain relationship', 'Backs up to · from Homelab map']);
  await kindPicker.selectOption({ label: 'Backs up to · from Homelab map' });
  await dialog.getByRole('textbox', { name: 'Relationship label' }).fill('nightly');
  await dialog.getByRole('button', { name: 'Save relationship', exact: true }).click();
  await expect(page.getByText('Relationship documented. No server configuration changed.')).toBeVisible();
  let edges = (await invoke('map_state')).edges;
  expect(edges.map(e => [e.source, e.target, e.label, e.kind])).toEqual([[host.id, backup.id, 'nightly', { plugin_id: MAP, kind_id: 'backs-up-to', title: 'Backs up to' }]]);
  const path0 = page.locator('.react-flow__edge-path').first();
  await expect(path0).toHaveAttribute('style', /stroke: (rgb\(91, 132, 177\)|#5b84b1).*stroke-dasharray: 7(px)?,? 5/);
  await expect(page.locator('.react-flow__edge-text').first()).toHaveText('Backs up to: nightly');
  // Not between servers it is not allowed on, and not through the plain path.
  expect(await rejected('add_relationship', { input: { source: host.id, target: third.id, label: 'x', kind: { plugin_id: MAP, contribution_id: 'backs-up-to' } } })).toBe('denied');
  expect(await rejected('add_relationship', { input: { source: host.id, target: backup.id, label: 'y', kind: { plugin_id: MAP, contribution_id: 'status-page' } } })).toBe('denied');
  expect((await invoke('map_state')).edges.length).toBe(1);
  await page.screenshot({ path: path.join(directory, 'relationship-kind.png') });
  // Disabling the plugin keeps the documented title and returns to the plain line.
  await invoke('plugin_disable', { pluginId: MAP });
  await expect(page.locator('.react-flow__edge-text').first()).toHaveText('Backs up to: nightly', { timeout: 10000 });
  await expect(path0).not.toHaveAttribute('style', /dasharray/, { timeout: 10000 });
  edges = (await invoke('map_state')).edges;
  expect(edges[0].kind.title).toBe('Backs up to');
  results.push('the relationship dialog offered "Backs up to" only between two servers the plugin is allowed on; the saved relationship carries the kind and is drawn as a dashed blue line labelled "Backs up to: nightly"; a kind between servers it is not allowed on, or a contribution that is not a kind, is refused; after disabling the plugin the label stays and the line is plain');

  // 7. Every open is in the plugin activity log.
  const log = (await invoke('plugin_activity', { limit: 500 })).entries;
  const opens = log.filter(e => e.action === 'open_app');
  expect(opens.filter(e => e.outcome === 'done' && e.detail === 'status-page').length).toBe(2);
  // A refused open is logged against the capability it was refused, below.
  expect(log.some(e => e.action === 'loopback_tunnel' && e.outcome === 'refused' && e.detail === 'trust_too_low')).toBe(true);
  expect(log.some(e => e.action === 'loopback_tunnel' && e.outcome === 'refused' && e.detail === 'not_granted')).toBe(true);
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  const activity = page.getByRole('region', { name: 'Plugin activity', exact: true }).getByRole('table', { name: 'Plugin activity log' });
  for (const text of ['Opened its application "status-page"', 'Opened through a private tunnel']) await expect(activity).toContainText(text);
  results.push('each application open is in the plugin activity log, and refusals name the reason (trust too low, not granted)');

  expect(pageErrors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'disposable loopback Linux container only', noSudo: true, noRealHost: true, pageErrors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await nav.getByRole('button', { name: 'Topology', exact: true }).click();
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native pack map proof failed in ${directory}.`);
  throw failure;
} finally {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(resolve => app.once('exit', resolve)); }
  if (server.exitCode === null) {
    server.stdin.end('\n');
    // The fixture removes its Podman container (bounded at 30 s) before exiting; killing it
    // sooner leaves the container running and slows every later fixture start.
    await Promise.race([new Promise(resolve => server.once('exit', resolve)), delay(60000)]);
    if (server.exitCode === null) server.kill();
  }
  lines.close();
}
