// Actual Windows Tauri/WebView2 proof for plugin packs (F21/T043): reviewed
// read-only commands run in a disposable Linux container over real OpenSSH.
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-pack-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const plugins = path.join(directory, 'plugins');
await mkdir(path.join(plugins, 'host-basics'), { recursive: true });
await copyFile(path.join(root, 'examples/host-basics/infrawerf-plugin.json'), path.join(plugins, 'host-basics/infrawerf-plugin.json'));
const CAPS = ['ssh_session', 'run_approved_command'];
const command = (id, title, argv, extra = {}) => ({ id, point: 'collector', title, capabilities: CAPS, command: { argv, extract: { kind: 'line' }, ...extra } });
const fixturePack = {
  id: 'org.example.pack-fixture', name: 'Pack fixture', version: '1.0.0', api_version: 2, capabilities: CAPS,
  contributions: [
    command('listing', 'Large listing', ['ls', '-lR', '/usr']),
    command('slow', 'Slow command', ['sleep', '45']),
    command('wait', 'Waiting command', ['sleep', '20']),
    { ...command('named', 'Service name', ['basename', '{unit}', '.service'], { parameters: [{ name: 'unit', source: 'service' }], healthy: ['atlas-fixture'] }), point: 'health_check' },
  ],
};
await mkdir(path.join(plugins, 'pack-fixture'), { recursive: true });
await writeFile(path.join(plugins, 'pack-fixture/infrawerf-plugin.json'), JSON.stringify(fixturePack, null, 2));
// A pack with a shell pipeline is never listed as a plugin.
await mkdir(path.join(plugins, 'unsafe'), { recursive: true });
await writeFile(path.join(plugins, 'unsafe/infrawerf-plugin.json'), JSON.stringify({ ...fixturePack, id: 'org.example.unsafe', contributions: [command('pipe', 'Pipe', ['sh', '-c', 'cat /etc/passwd | head'])] }));

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
    env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_SSH_CONFIG: info.config, ATLAS_PLUGINS_DIR: plugins, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9238 --remote-debugging-address=127.0.0.1' },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9238/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9238');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  const rejected = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args).then(() => null, e => e?.code ?? String(e)), { name, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();

  await invoke('add_server', { input: { name: 'Pack fixture host', host: 'atlas-linux' } });
  await invoke('add_server', { input: { name: 'Other host', host: 'other-fixture.home' } });
  const servers = (await invoke('map_state')).servers;
  const host = servers.find(s => s.host === 'atlas-linux'), other = servers.find(s => s.host !== 'atlas-linux');
  const BASICS = 'org.example.host-basics', FIXTURE = 'org.example.pack-fixture';
  const run = (pluginId, contributionId, parameters = {}, serverId = host.id) => ({ serverId, pluginId, contributionId, parameters });
  const observation = (state, plugin, id) => state.commands.find(c => c.plugin_id === plugin && c.contribution_id === id)?.observation ?? null;
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(2);
  const nav = page.getByRole('navigation', { name: 'Main navigation' });

  // 1. A found pack runs nothing and is not offered on any server.
  expect((await invoke('pack_state', { serverId: host.id })).commands).toEqual([]);
  expect(await rejected('pack_run', run(BASICS, 'kernel'))).toBe('denied');
  const state = await invoke('plugins_state');
  expect(state.plugins.map(p => p.id).sort()).toEqual([BASICS, FIXTURE]);
  expect(state.invalid.map(i => i.folder)).toEqual(['unsafe']);
  expect(state.invalid[0].problems.join(' ')).toContain('forbidden_program');
  results.push('a pack that was found but not enabled offers and runs nothing; a pack whose command is a shell pipeline is listed as unusable with forbidden_program');

  // 2. The owner reads the exact commands before enabling, then enables and allows one server.
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  const pluginsSection = page.getByRole('region', { name: 'Plugins', exact: true });
  await pluginsSection.getByRole('button', { name: 'Configure Host basics', exact: true }).click();
  const pluginPage = page.getByRole('region', { name: 'Host basics plugin', exact: true });
  await expect(pluginPage.getByLabel('Disk usage command', { exact: true })).toHaveText('df -P -k');
  await expect(pluginPage.getByLabel('Kernel version command', { exact: true })).toHaveText('uname -r');
  await expect(pluginPage.getByLabel('Service is active command', { exact: true })).toHaveText('systemctl is-active {unit}');
  await expect(pluginPage).toContainText('{unit} is a service you choose from that server’s inventory.');
  await expect(pluginPage).toContainText('Healthy when it is active.');
  await expect(pluginPage).toContainText('No shell reads the command, and it cannot use sudo.');
  await page.screenshot({ path: path.join(directory, 'pack-review.png') });
  await pluginPage.getByRole('radio', { name: /^Read-only/ }).check();
  await pluginPage.getByRole('button', { name: 'Enable plugin', exact: true }).click();
  await expect(pluginPage).toContainText('Status: Enabled · Read-only');
  // Enabled with one of the two permissions: still nothing.
  const permissions = pluginPage.getByRole('region', { name: 'Permissions on Pack fixture host', exact: true });
  await permissions.getByRole('checkbox', { name: /^Sign in to this server/ }).click();
  await expect(permissions.getByRole('checkbox', { name: /^Sign in to this server/ })).toBeChecked();
  expect((await invoke('pack_state', { serverId: host.id })).commands).toEqual([]);
  expect(await rejected('pack_run', run(BASICS, 'kernel'))).toBe('denied');
  await permissions.getByRole('checkbox', { name: /^Run fixed, read-only commands/ }).click();
  await expect(permissions.getByRole('checkbox', { name: /^Run fixed, read-only commands/ })).toBeChecked();
  expect((await invoke('pack_state', { serverId: host.id })).commands.map(c => c.contribution_id)).toEqual(['disk-usage', 'kernel', 'system-state', 'service-active']);
  expect((await invoke('pack_state', { serverId: other.id })).commands).toEqual([]);
  expect(await rejected('pack_run', run(BASICS, 'kernel', {}, other.id))).toBe('denied');
  results.push('the plugin page shows each exact command, its parameter and how it is read before enabling; commands are offered only after enabling and allowing both permissions, and only on that server');

  // 3. The collector and the health check appear for the granted server and run for real.
  await nav.getByRole('button', { name: 'Topology', exact: true }).click();
  await page.getByRole('button', { name: 'Open Pack fixture host workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Pack fixture host', exact: true });
  await workspace.getByRole('button', { name: 'Refresh inventory', exact: true }).click();
  await expect(workspace.getByRole('status')).toContainText('Approved inventory updated');
  await workspace.getByRole('tab', { name: 'Plugin data', exact: true }).click();
  const data = workspace.getByRole('region', { name: 'Plugin data', exact: true });
  const card = title => data.getByRole('article', { name: title, exact: true });
  await expect(data.getByRole('article')).toHaveCount(4);
  await expect(card('Kernel version')).toContainText('Collector');
  await expect(card('Kernel version')).toContainText('Not run yet.');
  await expect(card('System state')).toContainText('Health check');
  const kernelExpected = inContainer('uname', '-r').stdout.trim();
  expect(kernelExpected).toMatch(/^\d+\.\d+/);
  await card('Kernel version').getByRole('button', { name: 'Run Kernel version', exact: true }).click();
  await expect(card('Kernel version').locator('.pack-value')).toHaveText(kernelExpected, { timeout: 30000 });
  await expect(card('Kernel version')).toContainText('Fresh');
  await expect(card('Kernel version')).toContainText('Source: Host basics (org.example.host-basics.kernel) over your SSH access');
  await card('Disk usage').getByRole('button', { name: 'Run Disk usage', exact: true }).click();
  const table = card('Disk usage').getByRole('table', { name: 'Disk usage result' });
  await expect(table.locator('thead th')).toHaveText(['filesystem', 'blocks kib', 'used kib', 'available kib', 'used percent', 'mounted on'], { timeout: 30000 });
  await expect(table.locator('tbody tr').first()).toBeVisible();
  let packs = await invoke('pack_state', { serverId: host.id });
  const disk = observation(packs, BASICS, 'disk-usage');
  expect(disk.result.rows.some(row => row[5] === '/' && /^\d+%$/.test(row[4]))).toBe(true);
  expect(disk.result.rows.length).toBe(inContainer('df', '-P', '-k').stdout.trim().split('\n').length - 1);
  // Source, time and freshness, like a built-in observation.
  for (const id of ['kernel', 'disk-usage']) {
    const seen = observation(packs, BASICS, id);
    expect(seen.source).toBe(`${BASICS}.${id}`);
    expect(seen.source_binding).toMatch(/^src1_[0-9a-f]{64}$/);
    expect(Math.abs(Date.now() - Date.parse(seen.observed_at))).toBeLessThan(120000);
    expect(seen.fresh).toBe(true);
    expect(Object.keys(seen).sort()).toEqual(['fresh', 'observed_at', 'parameters', 'result', 'source', 'source_binding']);
  }
  // The container has no systemd, so this real health check has nothing to read: unknown, never healthy.
  await card('System state').getByRole('button', { name: 'Run System state', exact: true }).click();
  await expect(card('System state').locator('.pack-value')).toContainText('Unknown', { timeout: 30000 });
  await page.screenshot({ path: path.join(directory, 'plugin-data.png') });
  results.push(`a collector and a health check from the example pack appear in the granted server's Plugin data tab; uname -r returned the container's real kernel (${kernelExpected}) and df -P -k its real file systems as a table; each observation carries source, authenticated source binding, time and freshness; a health check with nothing readable is Unknown`);

  // 4. Parameters come only from what the authenticated session discovered.
  await invoke('plugin_enable', { pluginId: FIXTURE, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: FIXTURE, serverId: host.id, capabilities: CAPS });
  await workspace.getByRole('tab', { name: 'Services', exact: true }).click();
  await workspace.getByRole('tab', { name: 'Plugin data', exact: true }).click();
  await expect(data.getByRole('article')).toHaveCount(8);
  // Permission changes forget what packs observed.
  await expect(card('Kernel version')).toContainText('Not run yet.');
  const picker = card('Service name').getByLabel('Service name unit', { exact: true });
  const offered = await picker.locator('option').allTextContents();
  expect(offered).toContain('atlas-fixture.service');
  expect(offered).toContain('fixture-stopped.service');
  // User-scope services are not system services and are not offered.
  expect(offered).not.toContain('fixture-worker.service');
  await expect(card('Service name').getByRole('button', { name: 'Run Service name', exact: true })).toBeDisabled();
  await picker.selectOption({ label: 'atlas-fixture.service' });
  await card('Service name').getByRole('button', { name: 'Run Service name', exact: true }).click();
  await expect(card('Service name').locator('.pack-value')).toHaveText('atlas-fixture.service: Healthy: atlas-fixture', { timeout: 30000 });
  await picker.selectOption({ label: 'fixture-stopped.service' });
  await card('Service name').getByRole('button', { name: 'Run Service name', exact: true }).click();
  await expect(card('Service name').locator('.pack-value')).toHaveText('fixture-stopped.service: Unhealthy: fixture-stopped', { timeout: 30000 });
  // A name, shell text or an unknown ID in place of a discovered workload ID runs nothing.
  inContainer('rm', '-f', '/tmp/atlas-pack-injected');
  for (const value of ['atlas-fixture.service', '$(touch /tmp/atlas-pack-injected)', 'x; touch /tmp/atlas-pack-injected', 'wl1_' + '0'.repeat(64)]) {
    expect(await rejected('pack_run', run(FIXTURE, 'named', { unit: value }))).toBe('denied');
  }
  expect(await rejected('pack_run', run(FIXTURE, 'named', {}))).toBe('denied');
  expect(await rejected('pack_run', run(FIXTURE, 'listing', { extra: 'x' }))).toBe('denied');
  expect(inContainer('test', '-e', '/tmp/atlas-pack-injected').status).toBe(1);
  results.push('a parameter is chosen from the server\'s discovered system services; the health check is Healthy or Unhealthy by the declared values; names, shell text and unknown IDs in place of a discovered workload ID are refused and nothing ran');

  // 5. Output and run time are bounded by the broker that bounds built-in collectors.
  packs = await invoke('pack_run', run(FIXTURE, 'listing'));
  const listing = observation(packs, FIXTURE, 'listing');
  expect(inContainer('sh', '-c', 'ls -lR /usr | wc -c').stdout.trim() > 65536).toBe(true);
  expect(listing.result).toEqual({ outcome: 'output_limit', value: null, columns: [], rows: [], truncated: false, health: null });
  const started = Date.now();
  packs = await invoke('pack_run', run(FIXTURE, 'slow'));
  const elapsed = (Date.now() - started) / 1000;
  expect(observation(packs, FIXTURE, 'slow').result.outcome).toBe('timed_out');
  expect(elapsed).toBeGreaterThan(25);
  expect(elapsed).toBeLessThan(40);
  // The timed-out command does not keep running on the server.
  await expect.poll(() => inContainer('pgrep', '-x', 'sleep').status, { timeout: 10000 }).toBe(1);
  await workspace.getByRole('tab', { name: 'Services', exact: true }).click();
  await workspace.getByRole('tab', { name: 'Plugin data', exact: true }).click();
  await expect(card('Large listing')).toContainText('printed more than InfraWerf accepts');
  await expect(card('Slow command')).toContainText('ran longer than InfraWerf allows');
  results.push(`a command printing more than 64 KiB ends as output_limit with no value; a 45-second command ends as timed_out after ${elapsed.toFixed(1)} s, the 30-second limit of built-in collectors`);

  // 6. A run in flight is cancelled when the owner turns plugins off, and nothing is left behind.
  await invoke('pack_run', run(BASICS, 'kernel'));
  const pending = rejected('pack_run', run(FIXTURE, 'wait'));
  await delay(4000);
  expect(inContainer('pgrep', '-x', 'sleep').status).toBe(0);
  const switched = Date.now();
  await invoke('plugins_set_all_off', { off: true });
  expect(await pending).toBe('cancelled');
  expect(Date.now() - switched).toBeLessThan(8000);
  await expect.poll(() => inContainer('pgrep', '-x', 'sleep').status, { timeout: 10000 }).toBe(1);
  expect((await invoke('pack_state', { serverId: host.id })).commands).toEqual([]);
  expect(await rejected('pack_run', run(BASICS, 'kernel'))).toBe('denied');
  await invoke('plugins_set_all_off', { off: false });
  packs = await invoke('pack_state', { serverId: host.id });
  expect(packs.commands.length).toBe(8);
  expect(packs.commands.every(c => c.observation === null)).toBe(true);
  results.push('turning all plugins off cancels a pack command in flight within seconds, its remote process is gone, commands disappear from the server and earlier observations are forgotten');

  // 7. Every run is in the plugin activity log, and nothing a command printed is kept on disk.
  packs = await invoke('pack_run', run(BASICS, 'kernel'));
  expect(observation(packs, BASICS, 'kernel').result.value).toBe(kernelExpected);
  const log = (await invoke('plugin_activity', { limit: 500 })).entries;
  const runs = log.filter(e => e.action === 'run_command');
  const count = (outcome, detail) => runs.filter(e => e.outcome === outcome && e.detail === detail).length;
  expect(count('done', 'kernel')).toBe(3);
  expect(count('done', 'disk-usage')).toBe(1);
  expect(count('done', 'named')).toBe(2);
  // One run over the output limit, and one given a parameter it does not have.
  expect(count('failed', 'listing')).toBe(2);
  // The five runs given something other than a discovered workload.
  expect(count('failed', 'named')).toBe(5);
  expect(count('failed', 'slow')).toBe(1);
  expect(count('failed', 'system-state')).toBe(1);
  expect(count('cancelled', 'wait')).toBe(1);
  expect(runs.every(e => e.server_id === host.id && [BASICS, FIXTURE].includes(e.plugin_id))).toBe(true);
  expect(log.some(e => e.action === 'ssh_session' && e.outcome === 'refused' && e.detail === 'all_plugins_off')).toBe(true);
  expect(log.some(e => e.action === 'ssh_session' && e.outcome === 'refused' && e.detail === 'not_granted' && e.server_id === other.id)).toBe(true);
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  const activity = page.getByRole('region', { name: 'Plugin activity', exact: true }).getByRole('table', { name: 'Plugin activity log' });
  for (const text of ['Ran its command "kernel"', 'Finished', 'Ran its command "wait"', 'Cancelled: its permissions changed while it ran', 'Ran its command "listing"', 'Failed: nothing was read']) await expect(activity).toContainText(text);
  const stored = Buffer.concat(await Promise.all(['', '-wal'].map(suffix => readFile(database + suffix).catch(() => Buffer.alloc(0)))));
  for (const marker of [kernelExpected, 'atlas-fixture\n', disk.result.rows[0][0] + ' ', 'Mounted on']) expect(stored.includes(Buffer.from(marker))).toBe(false);
  expect(JSON.stringify(log)).not.toContain(kernelExpected);
  results.push('each run is in the plugin activity log as finished, failed or cancelled with its plugin, server and contribution; refusals are logged; no command output is in the log or the database');

  // 8. Withdrawing the permission removes the commands and refuses the next run.
  await invoke('plugin_set_grant', { pluginId: BASICS, serverId: host.id, capabilities: ['ssh_session'] });
  expect((await invoke('pack_state', { serverId: host.id })).commands.every(c => c.plugin_id === FIXTURE)).toBe(true);
  expect(await rejected('pack_run', run(BASICS, 'kernel'))).toBe('denied');
  await invoke('plugin_disable', { pluginId: FIXTURE });
  expect((await invoke('pack_state', { serverId: host.id })).commands).toEqual([]);
  await nav.getByRole('button', { name: 'Topology', exact: true }).click();
  await page.getByRole('button', { name: 'Open Pack fixture host workspace', exact: true }).click();
  await workspace.getByRole('tab', { name: 'Plugin data', exact: true }).click();
  await expect(data).toContainText('No plugin command is allowed on this server.');
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  results.push('withdrawing one permission or disabling the plugin removes its commands from the server and refuses the next run');

  expect(pageErrors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'disposable loopback Linux container only', noSudo: true, noRealHost: true, pageErrors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native pack proof failed in ${directory}.`);
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
