// Actual Windows Tauri/WebView2 proof for the reference plugin (F25/T051):
// examples/systemd-backups is installed through the normal install path,
// enabled read-only, and run against synthetic backup services in the
// disposable Linux container over real OpenSSH: its pack commands, its badge
// and its scheduled Rust job. Nothing on the server changes.
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-plugin-backups-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const plugins = path.join(directory, 'plugins');
await mkdir(plugins);
const PLUGIN = 'org.example.systemd-backups';
const MINUTE_SECONDS = 2;

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
  // The container's fake systemctl lists three synthetic backups only with this marker.
  expect(inContainer('touch', '/tmp/atlas-backup-fixture').status).toBe(0);
  const changes = () => inContainer('cat', '/tmp/atlas-control-count').stdout.trim() || '0';
  const database = path.join(directory, 'native.sqlite3');
  const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  app = spawn(process.env.ATLAS_DESKTOP_BINARY ? path.resolve(process.env.ATLAS_DESKTOP_BINARY) : path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root, windowsHide: true,
    env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_SSH_CONFIG: info.config, ATLAS_PLUGINS_DIR: plugins, ATLAS_SCHEDULE_MINUTE_SECONDS: String(MINUTE_SECONDS), WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9244 --remote-debugging-address=127.0.0.1' },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9244/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9244');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'Backup host', host: 'atlas-linux' } });
  const host = (await invoke('map_state')).servers[0];
  const changesBefore = changes();

  // 1. Installed through the normal install path, from the example's folder.
  const report = await invoke('plugin_install', { request: { source: { kind: 'folder', path: path.join(root, 'examples/systemd-backups') } } });
  await writeFile(path.join(directory, 'install.json'), JSON.stringify(report, null, 2));
  let plugin = (await invoke('plugins_state')).plugins.find(p => p.id === PLUGIN);
  expect([plugin.status, plugin.install.source_kind, plugin.grants.length]).toEqual(['disabled', 'folder', 0]);
  await page.reload();
  const nav = page.getByRole('navigation', { name: 'Main navigation' });
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('navigation', { name: 'Settings pages' }).getByRole('button', { name: 'Plugins', exact: true }).click();
  await page.getByRole('region', { name: 'Plugins', exact: true }).getByRole('button', { name: 'Configure Systemd backups', exact: true }).click();
  const pluginPage = page.getByRole('region', { name: 'Systemd backups plugin', exact: true });
  await expect(pluginPage).toContainText('systemctl list-units --all --type=service --state=failed,auto-restart --no-legend --no-pager --plain --full');
  await expect(pluginPage).toContainText('systemctl show --no-pager --value --property=Result {unit}');
  await expect(pluginPage).toContainText('job.wasm');
  await pluginPage.getByRole('radio', { name: /^Read-only/ }).check();
  await pluginPage.getByRole('button', { name: 'Enable plugin', exact: true }).click();
  await expect(pluginPage).toContainText('Status: Enabled · Read-only');
  await invoke('plugin_set_grant', { pluginId: PLUGIN, serverId: host.id, capabilities: ['read_saved_state', 'ssh_session', 'run_approved_command'] });
  await page.screenshot({ path: path.join(directory, 'plugin-page.png'), fullPage: true });
  results.push('the reference plugin installed from its folder through the normal install path, disabled until reviewed; its page showed each exact systemctl command and its job module; it was enabled read-only and allowed to read on one server');

  // 2. Its commands read the synthetic backups, through the Plugin data tab.
  await page.reload();
  await page.getByRole('button', { name: 'Open Backup host workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Backup host', exact: true });
  await workspace.getByRole('button', { name: 'Refresh inventory', exact: true }).click();
  await expect(workspace.getByRole('status')).toContainText('Approved inventory updated', { timeout: 60000 });
  await workspace.getByRole('tab', { name: 'Plugin data', exact: true }).click();
  const data = workspace.getByRole('region', { name: 'Plugin data', exact: true });
  const card = title => data.getByRole('article', { name: title, exact: true });
  await card('Failing services').getByRole('button', { name: 'Run Failing services', exact: true }).click();
  await expect(card('Failing services')).toContainText('fixture-backup.service', { timeout: 30000 });
  await expect(card('Failing services')).toContainText('fixture-loop-backup.service');
  await expect(card('Failing services')).not.toContainText('fixture-nightly-backup.service');
  await card('System state').getByRole('button', { name: 'Run System state', exact: true }).click();
  await expect(card('System state').locator('.pack-value')).toHaveText('Unhealthy: degraded', { timeout: 30000 });
  const picker = card('Last result').getByLabel('Last result unit', { exact: true });
  await picker.selectOption({ label: 'fixture-backup.service' });
  await card('Last result').getByRole('button', { name: 'Run Last result', exact: true }).click();
  await expect(card('Last result').locator('.pack-value')).toHaveText('fixture-backup.service: Unhealthy: exit-code', { timeout: 30000 });
  await picker.selectOption({ label: 'fixture-nightly-backup.service' });
  await card('Last result').getByRole('button', { name: 'Run Last result', exact: true }).click();
  await expect(card('Last result').locator('.pack-value')).toHaveText('fixture-nightly-backup.service: Healthy: success', { timeout: 30000 });
  const badge = (await invoke('pack_map')).badges.find(b => b.server_id === host.id);
  expect([badge.state, badge.value]).toEqual(['unhealthy', 'degraded']);
  await page.screenshot({ path: path.join(directory, 'plugin-data.png') });
  results.push('the plugin\'s commands listed the failed and the restarting backup and not the healthy one; System state read "degraded" and the badge shows it as unhealthy; Last result read exit-code for the failed backup and success for the healthy one');

  // 3. The Rust job checks every backup service, run now and on a schedule.
  const job = workspace.getByRole('article', { name: 'Check backups job' });
  await job.getByRole('button', { name: 'Run Check backups', exact: true }).click();
  const log = job.getByLabel('Check backups log');
  await expect(log).toContainText('3 backups checked, 2 failing', { timeout: 60000 });
  const expected = [
    'FAILING fixture-backup.service: exit-code, last finished Wed 2026-11-18 03:00:12 UTC, 0 restarts',
    'FAILING fixture-loop-backup.service: exit-code, last finished Wed 2026-11-18 09:41:07 UTC, 12 restarts',
    'ok fixture-nightly-backup.service: success, last finished Wed 2026-11-18 03:00:12 UTC, 0 restarts',
    '3 backups checked, 2 failing',
  ];
  let runs = (await invoke('plugin_jobs', { serverId: host.id })).runs;
  expect(runs[0].state).toBe('done');
  expect(runs[0].outcome.logs).toEqual(expected);
  expect(runs[0].outcome.result.failing).toBe(2);
  await job.screenshot({ path: path.join(directory, 'job.png') });
  await invoke('plugin_set_schedule', { serverId: host.id, pluginId: PLUGIN, contributionId: 'check-backups', everyMinutes: 5 });
  const first = runs[0].run_id;
  await expect.poll(async () => { const latest = (await invoke('plugin_jobs', { serverId: host.id })).runs[0]; return latest.run_id !== first && !latest.attended && latest.state !== 'running' ? latest.state : 'waiting'; }, { timeout: 60000 }).toBe('done');
  runs = (await invoke('plugin_jobs', { serverId: host.id })).runs;
  expect(runs[0].outcome.logs).toEqual(expected);
  await expect(job).toContainText('by its schedule');
  await invoke('plugin_set_schedule', { serverId: host.id, pluginId: PLUGIN, contributionId: 'check-backups', everyMinutes: null });
  results.push(`the plugin's Rust job found the three backup services in the saved inventory, read each one's last result, last finish and restart count through the plugin's own commands, and logged "${expected.at(-1)}" with one line per backup; a 5-minute schedule (a minute is ${MINUTE_SECONDS} s here) ran it again unattended at read-only trust with the same result`);

  // 4. It read everything and changed nothing.
  expect(changes()).toBe(changesBefore);
  const entries = (await invoke('plugin_activity', { limit: 500 })).entries.filter(e => e.plugin_id === PLUGIN);
  await writeFile(path.join(directory, 'activity.json'), JSON.stringify(entries, null, 2));
  expect(entries.some(e => e.action === 'job_run' && e.detail === 'check-backups: done')).toBe(true);
  expect(entries.some(e => e.action === 'job_run' && e.detail === 'check-backups: scheduled, done')).toBe(true);
  expect(entries.filter(e => e.outcome === 'refused')).toEqual([]);
  results.push('no workload control ran on the server, nothing the plugin asked was refused, and its runs are in the activity log');

  expect(pageErrors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'disposable loopback Linux container only', noSudo: true, noRealHost: true, pageErrors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native plugin backups proof failed in ${directory}.`);
  await page?.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
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
}
