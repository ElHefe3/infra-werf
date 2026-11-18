// End-to-end proof through the actual Tauri WebView2 and Rust IPC, on synthetic
// data only. The CDP listener exists only in this child process, on loopback.
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const directory = path.join(root, '.tools', `desktop-proof-${randomUUID()}`);
const fixture = spawnSync('python', ['scripts/desktop_fixture.py', directory], { cwd: root, encoding: 'utf8', windowsHide: true });
if (fixture.status !== 0) throw new Error(fixture.stderr || 'Fixture creation failed');
const config = JSON.parse(fixture.stdout);
const original = createHash('sha256').update(await readFile(config.source)).digest('hex');
const database = path.join(directory, 'native.sqlite3');
const sshConfig = path.join(directory, 'ssh_config');
await writeFile(sshConfig, 'Host *\n  BatchMode yes\n');
// T041: a private plugins folder with the shipped example and one unusable folder.
const pluginsDirectory = path.join(directory, 'plugins');
await mkdir(path.join(pluginsDirectory, 'backup-overview'), { recursive: true });
await copyFile(path.join(root, 'examples', 'backup-overview', 'infrawerf-plugin.json'), path.join(pluginsDirectory, 'backup-overview', 'infrawerf-plugin.json'));
await mkdir(path.join(pluginsDirectory, 'broken'), { recursive: true });
await writeFile(path.join(pluginsDirectory, 'broken', 'infrawerf-plugin.json'), '{ not json');
const binary = process.env.ATLAS_DESKTOP_BINARY
  ? path.resolve(process.env.ATLAS_DESKTOP_BINARY)
  : path.join(root, 'target', 'debug', 'atlas-native.exe');
const endpoint = 'http://127.0.0.1:9225';
let child, browser, page;
const errors = [];
const results = [];
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function cdpAvailable() {
  try { return (await fetch(`${endpoint}/json/version`)).ok; } catch { return false; }
}
async function waitForCdpState(available, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await cdpAvailable() === available) return;
    await delay(250);
  }
  throw new Error(`WebView CDP endpoint did not become ${available ? 'available' : 'unavailable'} within ${timeout}ms`);
}
async function launch(db = database, legacy = config.source, extra = {}) {
  child = spawn(binary, [], { cwd: root, windowsHide: true, env: { ...process.env, ...extra, ATLAS_PLUGINS_DIR: pluginsDirectory, ATLAS_SSH_CONFIG:sshConfig, ATLAS_NATIVE_DB: db, ATLAS_LEGACY_DB: legacy, ATLAS_REPORTS_DIR: config.reports, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9225 --remote-debugging-address=127.0.0.1', WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview-profile') }, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.on('data', () => {}); child.stderr.on('data', () => {});
  // Tauri's Windows process may return success after handing startup to the
  // GUI process. The WebView endpoint, not the short-lived launcher handle,
  // determines whether the native window is ready.
  await waitForCdpState(true);
  browser = await chromium.connectOverCDP(endpoint);
  const context = browser.contexts()[0];
  page = context.pages()[0] || await context.waitForEvent('page');
  await page.waitForLoadState('domcontentloaded');
  page.on('pageerror', error => errors.push(error.message));
}
async function stop() {
  if (page && !page.isClosed()) {
    try {
      const quit = page.getByRole('button', { name: 'Quit InfraWerf', exact: true });
      if (await quit.count()) {
        await quit.click({ timeout: 2000 });
        await waitForCdpState(false, 7000);
      }
    } catch {}
  }
  if (browser) { await browser.close().catch(() => {}); browser = null; }
  if (child && child.exitCode === null) { child.kill(); await new Promise(resolve => child.once('exit', resolve)); }
  child = null; page = null; await delay(300);
}
async function invoke(command, args) {
  return page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
}
async function fitViewport(width, height) {
  await page.setViewportSize({ width, height });
  await expect(page.getByRole('button', { name: 'Add server', exact: false })).toBeVisible();
  const layout = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, scrollWidth: document.documentElement.scrollWidth, scrollHeight: document.documentElement.scrollHeight, footer: document.querySelector('.map-panel').getBoundingClientRect().bottom, map: document.querySelector('.map-canvas')?.getBoundingClientRect().height }));
  expect(layout.scrollWidth).toBeLessThanOrEqual(width);
  expect(layout.scrollHeight).toBeLessThanOrEqual(height);
  expect(layout.footer).toBeLessThanOrEqual(height + 1);
  expect(layout.map).toBeGreaterThan(200);
  await page.getByRole('button', { name: 'Fit map', exact: true }).click();
  await page.screenshot({ path: path.join(directory, `desktop-${width}x${height}.png`) });
  results.push(`desktop fit ${width}×${height}`);
}

try {
  await launch();
  await expect(page.locator('.server-card')).toHaveCount(4);
  const baseline = await invoke('map_state');
  expect(baseline.servers).toHaveLength(4);
  expect(JSON.stringify(baseline)).not.toContain('synthetic-excluded-marker');
  expect(baseline.servers.every(s => s.reachability.state === 'unknown')).toBe(true);
  await fitViewport(1280, 720);
  await fitViewport(1440, 900);
  // T053: categorised main menu and a retractable menu (category pop-outs disabled, D064).
  const nav = page.getByRole('navigation', { name: 'Main navigation' });
  for (const [category, items] of [['Map', ['Topology', 'Inventory']], ['Operations', ['Transfers', 'Activity']], ['Manage', ['Servers & monitoring', 'Settings']]]) {
    const region = nav.getByRole('region', { name: category, exact: true });
    await expect(region.getByRole('listitem')).toHaveCount(items.length);
    for (const item of items) await expect(region.getByRole('button', { name: item, exact: true })).toBeVisible();
  }
  await expect(nav.locator('ul ul')).toHaveCount(0);
  const mapRegion = nav.getByRole('region', { name: 'Map', exact: true });
  await mapRegion.getByRole('button', { name: 'Map category', exact: true }).click();
  await expect(mapRegion.getByRole('listitem')).toHaveCount(0);
  await mapRegion.getByRole('button', { name: 'Map category', exact: true }).click();
  await expect(mapRegion.getByRole('listitem')).toHaveCount(2);
  // Pop-out category copies are disabled (D064): no detach control and no floating panel.
  await expect(nav.getByRole('button', { name: /^(Detach|Dock) / })).toHaveCount(0);
  await expect(page.locator('.menu-panel')).toHaveCount(0);
  // Hide the main menu; the choice survives a reload, and Ctrl+B or one button brings it back.
  await page.getByRole('button', { name: 'Hide menu', exact: true }).click();
  await expect(nav).toHaveCount(0);
  expect((await page.locator('.app-main').boundingBox()).x).toBe(0);
  await expect(page.getByRole('button', { name: 'Show menu', exact: true })).toBeVisible();
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(4);
  await expect(nav).toHaveCount(0);
  await page.keyboard.press('Control+b');
  await expect(nav).toBeVisible();
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Menu layout', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Hide menu', exact: true }).click();
  await page.getByRole('button', { name: 'Reset menu layout', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Menu layout reset.');
  await expect(page.locator('.menu-panel')).toHaveCount(0);
  await expect(nav).toBeVisible();
  await nav.getByRole('button', { name: 'Topology', exact: true }).click();
  await expect(page.locator('.server-card')).toHaveCount(4);
  expect(errors).toEqual([]);
  // T041: plugin permissions. Found plugins have no trust and no grants; the owner chooses both on the plugin's page, and the core enforces them.
  const PLUGIN = 'org.example.backup-overview';
  const fixtureServers = (await invoke('map_state')).servers;
  const core = fixtureServers.find(s => s.name === 'Core & data'), other = fixtureServers.find(s => s.name !== 'Core & data');
  const ask = async (server, capability) => { const decision = await invoke('plugin_authorize', { pluginId: PLUGIN, serverId: server.id, capability }); return decision.allowed ? (decision.needs_confirmation ? 'confirm' : 'allowed') : decision.refusal; };
  expect(await ask(core, 'ssh_session')).toBe('plugin_disabled');
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  const pluginsSection = page.getByRole('region', { name: 'Plugins', exact: true });
  const installed = pluginsSection.getByRole('list', { name: 'Installed plugins' });
  await expect(installed.getByRole('listitem')).toHaveCount(1);
  await expect(installed).toContainText('Backup overview');
  await expect(installed).toContainText('Not enabled · allowed on 0 servers');
  const unusable = pluginsSection.getByRole('list', { name: 'Unusable plugin folders' });
  await expect(unusable).toContainText('broken');
  await expect(unusable).toContainText('malformed_json');
  await pluginsSection.getByRole('button', { name: 'Configure Backup overview', exact: true }).click();
  const pluginPage = page.getByRole('region', { name: 'Backup overview plugin', exact: true });
  // Consent: every requested capability in plain language, and no trust level chosen for the owner.
  const asks = pluginPage.getByRole('list', { name: 'Requested capabilities' });
  await expect(asks.getByRole('listitem')).toHaveCount(3);
  await expect(asks).toContainText('Sign in to this server through InfraWerf with your existing SSH access. The plugin never receives your keys or a raw session.');
  await expect(asks).toContainText('Run fixed, read-only commands on this server.');
  await expect(asks).toContainText('Used by: Last backup, Backups');
  // The trust choice; the Update form has its own source radios.
  const trustChoice = pluginPage.getByRole('group', { name: 'Choose how far this plugin may act. Nothing is chosen for you.' });
  await expect(trustChoice.getByRole('radio')).toHaveCount(3);
  await expect(trustChoice.getByRole('radio', { checked: true })).toHaveCount(0);
  await expect(pluginPage.getByRole('button', { name: 'Enable plugin', exact: true })).toBeDisabled();
  await pluginPage.getByRole('radio', { name: /^Read-only/ }).check();
  await pluginPage.getByRole('button', { name: 'Enable plugin', exact: true }).click();
  await expect(pluginPage).toContainText('Status: Enabled · Read-only');
  // Enabled but nothing granted: still refused. Grant one capability on one server.
  expect(await ask(core, 'ssh_session')).toBe('not_granted');
  const corePermissions = pluginPage.getByRole('region', { name: 'Permissions on Core & data', exact: true });
  await expect(corePermissions).toContainText('0 of 3 allowed');
  // The box is ticked once the core has saved the grant, not on the click itself.
  await corePermissions.getByRole('checkbox', { name: /^Sign in to this server/ }).click();
  await expect(corePermissions.getByRole('checkbox', { name: /^Sign in to this server/ })).toBeChecked();
  await expect(corePermissions).toContainText('1 of 3 allowed');
  expect(await ask(core, 'ssh_session')).toBe('allowed');
  await pluginPage.scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(directory, 'plugin-page.png') });
  expect(await ask(core, 'run_approved_command')).toBe('not_granted');
  expect(await ask(other, 'ssh_session')).toBe('not_granted');
  expect(await ask(core, 'write_files')).toBe('not_requested');
  expect(await ask(core, 'elevate')).toBe('not_requested');
  expect(await ask(core, 'interactive_terminal')).toBe('not_requested');
  // The server's own Settings tab lists it read-only and links back to the plugin's page.
  await nav.getByRole('button', { name: 'Topology', exact: true }).click();
  await page.getByRole('button', { name: 'Open Core & data workspace', exact: true }).click();
  const pluginWorkspace = page.getByRole('dialog', { name: 'Core & data', exact: true });
  await pluginWorkspace.getByRole('tab', { name: 'Settings', exact: true }).click();
  const serverPlugins = pluginWorkspace.getByRole('region', { name: 'Plugins on this server', exact: true });
  await expect(serverPlugins).toContainText('Backup overview');
  await expect(serverPlugins).toContainText('Enabled · Read-only');
  await expect(serverPlugins).toContainText('ssh_session');
  await expect(serverPlugins.getByRole('checkbox')).toHaveCount(0);
  await expect(serverPlugins.getByRole('radio')).toHaveCount(0);
  await serverPlugins.getByRole('button', { name: 'Open Backup overview plugin settings', exact: true }).click();
  await expect(pluginWorkspace).not.toBeVisible();
  await expect(pluginPage).toBeVisible();
  // Withdraw: access stops at once. Allow everything, then disable: everything stops and the trust choice is forgotten.
  await corePermissions.getByRole('button', { name: 'Withdraw all on Core & data', exact: true }).click();
  await expect(corePermissions).toContainText('0 of 3 allowed');
  expect(await ask(core, 'ssh_session')).toBe('not_granted');
  await corePermissions.getByRole('button', { name: 'Allow all on Core & data', exact: true }).click();
  await expect(corePermissions).toContainText('3 of 3 allowed');
  expect(await ask(core, 'run_approved_command')).toBe('allowed');
  await pluginPage.getByRole('button', { name: 'Disable plugin', exact: true }).click();
  await expect(pluginPage).toContainText('Status: Not enabled');
  expect(await ask(core, 'run_approved_command')).toBe('plugin_disabled');
  await expect(pluginPage.getByRole('group', { name: 'Choose how far this plugin may act. Nothing is chosen for you.' }).getByRole('radio', { checked: true })).toHaveCount(0);
  await expect(pluginPage.getByRole('button', { name: 'Enable plugin', exact: true })).toBeDisabled();
  // Trust levels differ only for changes; this example has none, so confirm the stored level round-trips, then clean up.
  await pluginPage.getByRole('radio', { name: /^Trusted automation/ }).check();
  await pluginPage.getByRole('button', { name: 'Enable plugin', exact: true }).click();
  await expect(pluginPage).toContainText('Status: Enabled · Trusted automation');
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(4);
  expect((await invoke('plugins_state')).plugins[0]).toMatchObject({ id: PLUGIN, status: 'enabled', trust: 'trusted_automation' });
  // T042: the kill switch, revoking trusted automation and the plugin activity log.
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(installed).toContainText('Enabled · Trusted automation · allowed on 1 server');
  expect(await ask(core, 'ssh_session')).toBe('allowed');
  await pluginsSection.getByRole('button', { name: 'Turn all plugins off', exact: true }).click();
  await expect(pluginsSection.getByRole('alert')).toContainText('All plugins are turned off.');
  await expect(installed).toContainText('Off with all plugins');
  expect(await ask(core, 'ssh_session')).toBe('all_plugins_off');
  // The switch survives a reload, and turning plugins back on restores exactly what was set.
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(4);
  expect((await invoke('plugins_state')).all_off).toBe(true);
  expect(await ask(core, 'run_approved_command')).toBe('all_plugins_off');
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  await pluginsSection.getByRole('button', { name: 'Turn plugins back on', exact: true }).click();
  await expect(installed).toContainText('Enabled · Trusted automation · allowed on 1 server');
  expect(await ask(core, 'ssh_session')).toBe('allowed');
  // Trusted automation is revoked without disabling the plugin.
  await pluginsSection.getByRole('button', { name: 'Configure Backup overview', exact: true }).click();
  await pluginPage.getByRole('button', { name: 'Revoke trusted automation', exact: true }).click();
  await expect(pluginPage).toContainText('Status: Enabled · Changes with confirmation');
  await expect(pluginPage.getByRole('button', { name: 'Revoke trusted automation', exact: true })).toHaveCount(0);
  expect((await invoke('plugins_state')).plugins[0]).toMatchObject({ status: 'enabled', trust: 'confirm_changes' });
  expect(await ask(core, 'ssh_session')).toBe('allowed');
  await expect(pluginPage.getByRole('table', { name: 'Backup overview recent activity' })).toContainText('Trusted automation revoked by you');
  // The log: every call with plugin, server, action and outcome, and every change the owner made.
  const activitySection = page.getByRole('region', { name: 'Plugin activity', exact: true });
  await activitySection.getByRole('button', { name: 'Refresh activity', exact: true }).click();
  const activityTable = activitySection.getByRole('table', { name: 'Plugin activity log' });
  for (const text of ['Backup overview', 'Core & data', 'Asked to use ssh_session', 'Allowed', 'Refused: all plugins are turned off', 'Refused: it is not allowed on this server', 'All plugins turned off by you', 'Plugins turned back on by you', 'Enabled by you', 'Permissions set by you']) await expect(activityTable).toContainText(text);
  await page.screenshot({ path: path.join(directory, 'plugin-activity.png') });
  const activity = await invoke('plugin_activity', { limit: 500 });
  expect(activity.kept).toBe((await invoke('monitoring_state')).history_limit);
  const calls = activity.entries.filter(e => !['enable', 'disable', 'set_permissions', 'revoke_trusted_automation', 'all_plugins_off', 'all_plugins_on'].includes(e.action));
  expect(calls.filter(e => e.outcome === 'refused').length).toBe(11);
  expect(calls.filter(e => e.outcome === 'allowed').length).toBe(5);
  expect(calls.every(e => e.plugin_id === PLUGIN && e.plugin_name === 'Backup overview' && [core.id, other.id].includes(e.server_id) && e.action && e.at)).toBe(true);
  expect(activity.entries.filter(e => e.action.startsWith('all_plugins_')).map(e => e.action)).toEqual(['all_plugins_on', 'all_plugins_off']);
  expect(activity.entries.every(e => Object.keys(e).sort().join() === 'action,at,detail,id,outcome,plugin_id,plugin_name,server_id')).toBe(true);
  expect(JSON.stringify(activity)).not.toMatch(/password|passphrase|BEGIN|PRIVATE|token|secret/i);
  // Plugin entries no longer go into the general history.
  expect((await invoke('map_state')).events.some(e => e.message.includes('Backup overview'))).toBe(false);
  await invoke('plugin_set_grant', { pluginId: PLUGIN, serverId: core.id, capabilities: [] });
  await invoke('plugin_disable', { pluginId: PLUGIN });
  expect(errors).toEqual([]);
  results.push('plugins: found with no trust and no grants; consent lists each capability in plain language; enabling needs an explicit trust level; grant, use, withdraw and disable are enforced in the core per server and logged; the server Settings tab shows a read-only list that links to the plugin page');
  results.push('plugin activity log records every call with plugin, server, action and outcome plus the owner\'s changes, within the history bound and with no free text; the kill switch refuses everything at once, survives a reload and restores settings; trusted automation is revoked without disabling');
  results.push('main menu groups items in one-level categories; category pop-outs are disabled (D064); the menu hides and the floating button or Ctrl+B restores it; Settings resets the layout');
  await page.getByRole('button', { name: 'Inventory', exact: true }).first().click();
  await page.getByRole('textbox', { name: 'Find a server or workload' }).fill('unit-539');
  await expect(page.getByText('unit-539.service', { exact: true })).toBeVisible();
  await expect(page.locator('tbody tr')).toHaveCount(1);
  await page.getByRole('button', { name: 'Clear search' }).click();
  await page.getByRole('button', { name: 'Topology', exact: true }).first().click();
  await page.keyboard.press('/');
  await expect(page.getByRole('textbox', { name: 'Find a server or workload' })).toBeFocused();
  await page.getByRole('textbox', { name: 'Find a server or workload' }).fill('unit-539');
  await expect(page.locator('.server-card.dimmed')).toHaveCount(3);
  await page.getByRole('button', { name: 'Clear search' }).click();
  results.push('full 540-service inventory, nested search and keyboard shortcut');

  // React Flow's keyboard selection must update the same inspector/edit target;
  // arrow movements must persist, just like pointer drags.
  const keyboardServer = baseline.servers[1];
  const keyboardNode = page.locator(`.react-flow__node[data-id="${keyboardServer.id}"]`);
  await keyboardNode.focus(); await keyboardNode.press('Enter');
  await expect(page.locator('.inspector h2')).toHaveText(keyboardServer.name);
  await keyboardNode.press('ArrowRight');
  await expect.poll(async () => (await invoke('map_state')).servers.find(s => s.id === keyboardServer.id).x).toBe(keyboardServer.x + 20);
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(4);
  expect((await invoke('map_state')).servers.find(s => s.id === keyboardServer.id).x).toBe(keyboardServer.x + 20);
  results.push('keyboard selection synchronises inspector; arrow movement persists after reload; group selection disabled');

  await page.getByRole('button', { name: 'Add server', exact: false }).click();
  let dialog = page.getByRole('dialog', { name: 'Add a server' });
  await dialog.getByLabel('Server name').fill('Desktop proof');
  await dialog.getByLabel('Host or IP address').fill('proof.fixture.invalid');
  await dialog.getByRole('button', { name: 'Save server' }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.locator('.server-card')).toHaveCount(5);
  const added = (await invoke('map_state')).servers.find(s => s.host === 'proof.fixture.invalid');
  await page.locator(`[data-server-id="${added.id}"]`).click();
  await page.getByRole('button', { name: 'Edit details', exact: true }).click();
  dialog = page.getByRole('dialog', { name: 'Edit server', exact: true });
  await dialog.getByLabel('Server name').fill('Desktop renamed');
  await dialog.getByRole('button', { name: 'Save server' }).click();
  await expect(dialog).not.toBeVisible();
  // Duplicate-host failure must retain the form and the stored map.
  await page.getByRole('button', { name: 'Add server', exact: false }).click();
  dialog = page.getByRole('dialog', { name: 'Add a server' });
  await dialog.getByLabel('Server name').fill('Duplicate');
  await dialog.getByLabel('Host or IP address').fill('proof.fixture.invalid');
  await dialog.getByRole('button', { name: 'Save server' }).click();
  await expect(dialog.getByRole('alert')).toContainText('already exists');
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  expect((await invoke('map_state')).servers).toHaveLength(5);
  results.push('add/edit through real IPC; duplicate host fails without losing form/data');

  await page.getByRole('button', { name: 'Document relationship', exact: false }).click();
  dialog = page.getByRole('dialog', { name: 'Document a relationship' });
  await dialog.getByLabel('From', { exact: true }).selectOption(added.id);
  await dialog.getByLabel('To', { exact: true }).selectOption(baseline.servers[0].id);
  await dialog.getByLabel('Relationship label').fill('Desktop proof link');
  await dialog.getByRole('button', { name: 'Save relationship' }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.locator('.react-flow__edge')).toHaveCount(1);
  const beforeDrag = await invoke('map_state');
  const card = page.locator(`[data-server-id="${added.id}"] h3`);
  await page.getByRole('button', { name: 'Fit map' }).click();
  await delay(250);
  const box = await card.boundingBox();
  await page.mouse.move(box.x + 20, box.y + 7);
  await page.mouse.down(); await page.mouse.move(box.x + 95, box.y + 42, { steps: 15 }); await page.mouse.up();
  await expect.poll(async () => (await invoke('map_state')).servers.find(s => s.id === added.id).x).not.toBe(beforeDrag.servers.find(s => s.id === added.id).x);
  const saved = await invoke('map_state');
  const moved = saved.servers.find(s => s.id === added.id);
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(5);
  expect((await invoke('map_state')).servers.find(s => s.id === added.id)).toEqual(moved);
  expect((await invoke('map_state')).edges).toEqual(saved.edges);
  results.push('directed labelled relationship and dragged coordinates survive renderer reload');

  const mapTransform = await page.locator('.react-flow__viewport').getAttribute('style');
  await page.getByRole('button', { name: 'Zoom in', exact: true }).click();
  await expect(page.locator('.react-flow__viewport')).not.toHaveAttribute('style', mapTransform);
  const beforePan = await page.locator('.react-flow__viewport').getAttribute('style');
  const canvas = await page.locator('.map-canvas').boundingBox();
  await page.mouse.move(canvas.x + 30, canvas.y + 100);
  await page.mouse.down(); await page.mouse.move(canvas.x + 90, canvas.y + 125, { steps: 12 }); await page.mouse.up();
  await expect(page.locator('.react-flow__viewport')).not.toHaveAttribute('style', beforePan);
  expect((await invoke('map_state')).servers).toEqual(saved.servers);
  // Tauri's close API exercises the native close event. DOM window.close()
  // is not a native-window request in WebView2.
  const pid = child.pid;
  const second = spawn(binary, [], { cwd: root, windowsHide: true, stdio: 'ignore' });
  await new Promise((resolve, reject) => { const timeout = setTimeout(() => reject(new Error('Second instance did not exit')), 8000); second.once('exit', code => { clearTimeout(timeout); expect(code).toBe(0); resolve(); }); });
  expect(child.pid).toBe(pid);
  await page.getByRole('button', { name: 'Quit InfraWerf', exact: true }).click();
  await waitForCdpState(false, 10000);
  await stop();
  await launch();
  await expect(page.locator('.server-card')).toHaveCount(5);
  expect((await invoke('map_state')).servers).toEqual(saved.servers);
  expect((await invoke('map_state')).edges).toEqual(saved.edges);
  results.push('pan/zoom leave layout intact; second instance exits; native close/relaunch persists edits');

  await page.getByRole('button', { name: 'Open Core & data workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Core & data', exact: true });
  await workspace.getByLabel('Filter saved workloads').fill('unit-539');
  await expect(workspace.getByText('unit-539.service', { exact: true })).toBeVisible();
  await workspace.getByRole('tab', { name: 'Files', exact: true }).click();
  await expect(workspace.getByRole('button', { name: 'Open file session', exact: true })).toBeVisible();
  await expect(workspace.locator('.file-list')).toHaveCount(0);
  await workspace.getByRole('tab', { name: 'Terminal', exact: true }).click();
  await expect(workspace.getByText('A protected SSH session is required.', { exact: true })).toBeVisible();
  await workspace.getByRole('tab', { name: 'System', exact: true }).click();
  await expect(workspace.getByText('16 GiB', { exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(workspace).not.toBeVisible();
  await page.locator(`[data-server-id="${added.id}"]`).click();
  await page.getByRole('button', { name: 'Relationships', exact: true }).click();
  await page.getByRole('button', { name: 'Remove relationship', exact: true }).click();
  await page.getByRole('dialog', { name: 'Remove relationship?', exact: true }).getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(page.locator('.react-flow__edge')).toHaveCount(0);
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(5);
  expect((await invoke('map_state')).edges).toHaveLength(0);
  results.push('workspace tabs/full inventory, Files requires explicit authentication, other unavailable controls stay honest, explicit edge removal persists');

  // T038: remote applications is a built-in extension. Switched off, its tab and commands are gone and nothing else changes.
  const tabNames = async () => { await page.getByRole('button', { name: 'Open Core & data workspace', exact: true }).click(); const dialog = page.getByRole('dialog', { name: 'Core & data', exact: true }); await expect(dialog.getByRole('tab', { name: 'Services', exact: true })).toBeVisible(); const names = await dialog.getByRole('tab').allTextContents(); await page.keyboard.press('Escape'); await expect(dialog).not.toBeVisible(); return names.map(name => name.trim()); };
  const rejected = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args).then(() => null, error => error?.code ?? JSON.stringify(error)), { command, args });
  const enabledCatalogue = await invoke('extensions_state');
  const enabledTabs = await tabNames();
  expect(enabledTabs).toContain('Apps');
  expect(await invoke('tunnel_list')).toEqual([]);
  const mapBefore = await invoke('map_state');
  await stop();
  await launch(database, config.source, { ATLAS_DISABLED_EXTENSIONS: 'builtin.remote-apps' });
  await expect(page.locator('.server-card')).toHaveCount(5);
  const disabledCatalogue = await invoke('extensions_state');
  expect(disabledCatalogue.extensions).toEqual([{ id: 'builtin', title: 'InfraWerf core', enabled: true, optional: false }, { id: 'builtin.remote-apps', title: 'Remote applications', enabled: false, optional: true }]);
  const removed = enabledCatalogue.contributions.filter(c => !disabledCatalogue.contributions.some(d => d.id === c.id));
  expect(removed.map(c => c.id).sort()).toEqual(['builtin.operation.application-discovery', 'builtin.operation.loopback-tunnel', 'builtin.tab.apps']);
  expect(removed.every(c => c.owner === 'builtin.remote-apps')).toBe(true);
  expect(disabledCatalogue.contributions).toEqual(enabledCatalogue.contributions.filter(c => c.owner !== 'builtin.remote-apps'));
  expect(await tabNames()).toEqual(enabledTabs.filter(name => name !== 'Apps'));
  const anyServer = mapBefore.servers[0].id;
  for (const [command, args] of [['tunnel_list'], ['tunnel_close_all'], ['tunnel_close', { tunnelId: 'none' }], ['tunnel_focus', { tunnelId: 'none' }], ['tunnel_discover', { serverId: anyServer }], ['tunnel_open', { serverId: anyServer, request: { remote_port: 8080, path: '/' } }]]) expect(await rejected(command, args), command).toBe('not_found');
  const mapAfter = await invoke('map_state');
  for (const key of ['servers', 'edges', 'events']) expect(mapAfter[key], key).toEqual(mapBefore[key]);
  expect(errors).toEqual([]);
  await stop();
  await launch();
  await expect(page.locator('.server-card')).toHaveCount(5);
  expect((await invoke('extensions_state')).contributions).toEqual(enabledCatalogue.contributions);
  results.push('remote applications switched off as a built-in extension: the Apps tab, its two operations and all six tunnel commands are gone, every other tab and contribution is unchanged, and re-enabling restores them');
  await page.screenshot({ path: path.join(directory, 'desktop-final.png') });
  await stop();

  const bad = path.join(directory, 'incompatible.sqlite3');
  await writeFile(bad, 'synthetic incompatible database');
  const beforeBad = await readFile(bad);
  await launch(bad);
  await expect(page.getByRole('alert')).toBeVisible();
  await page.getByRole('button', { name: 'Retry load', exact: true }).click();
  await expect(page.getByText('Your workspace couldn’t be opened.', { exact: true })).toBeVisible();
  expect(await readFile(bad)).toEqual(beforeBad);
  await page.screenshot({ path: path.join(directory, 'desktop-startup-error.png') });
  results.push('startup error/retry visible; incompatible file never replaced by empty data');
  expect(createHash('sha256').update(await readFile(config.source)).digest('hex')).toBe(original);
  expect(errors).toEqual([]);
  const proof = { result: 'pass', platform: 'Windows Tauri / WebView2 / real Rust IPC', results, consoleErrors: errors, legacyUnchanged: true, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
} catch (error) {
  if (page) await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
  console.error(`Desktop proof failed; artifacts: ${directory}`);
  throw error;
} finally { await stop(); }
