// Actual Windows Tauri/WebView2 proof for plugin updates (F22/T046). Each
// case has the plugin at two versions, served as zips by the loopback HTTPS
// fixture. An update asking for nothing new keeps working; anything new
// switches the plugin off until reviewed and grants nothing; a failed update
// leaves the previous version working.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-plugin-update-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const plugins = path.join(directory, 'plugins');
await mkdir(plugins);
const updates = path.join(root, 'crates/atlas-core/tests/fixtures/plugin-updates');
const PACK = 'org.example.host-basics';
const BOTH = ['ssh_session', 'run_approved_command'];

const fixture = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/plugin_https_fixture.py', path.join(directory, 'https')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
const lines = createInterface({ input: fixture.stdout });
let diagnostic = '';
fixture.stderr.on('data', data => { diagnostic += data.toString().slice(0, 2000); });
const https = await new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(new Error(`HTTPS fixture startup failed: ${diagnostic}`)), 30000);
  lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); });
  fixture.once('exit', code => { clearTimeout(timer); reject(new Error(`HTTPS fixture exited ${code}: ${diagnostic}`)); });
});
const address = (c, v = 'v2') => `https://127.0.0.1:${https.port}/updates/${c}-${v}.zip`;
const database = path.join(directory, 'native.sqlite3');
const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
let app, browser, page;
const pageErrors = [], results = [];
try {
  app = spawn(process.env.ATLAS_DESKTOP_BINARY ? path.resolve(process.env.ATLAS_DESKTOP_BINARY) : path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root, windowsHide: true,
    env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_PLUGINS_DIR: plugins, ATLAS_PLUGIN_TEST_CA: https.ca, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9241 --remote-debugging-address=127.0.0.1' },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9241/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9241');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  page.on('pageerror', error => pageErrors.push(error.message));
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  await invoke('add_server', { input: { name: 'Update host', host: 'update-fixture.home' } });
  const server = (await invoke('map_state')).servers[0];
  const allowed = async capability => (await invoke('plugin_authorize', { pluginId: PACK, serverId: server.id, capability }));
  const view = async () => (await invoke('plugins_state')).plugins.find(p => p.id === PACK);
  const folderBytes = async () => Promise.all((await readdir(path.join(plugins, PACK))).sort().map(async n => [n, createHash('sha256').update(await readFile(path.join(plugins, PACK, n))).digest('hex')]));

  // Version 1, installed, enabled read-only and allowed on the server.
  expect((await invoke('plugin_install', { request: { source: { kind: 'folder', path: path.join(updates, 'same', 'v1') } } })).problem).toBe(null);
  await invoke('plugin_enable', { pluginId: PACK, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: PACK, serverId: server.id, capabilities: BOTH });
  expect((await allowed('ssh_session')).allowed).toBe(true);

  const nav = page.getByRole('navigation', { name: 'Main navigation' });
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  const section = page.getByRole('region', { name: 'Plugins', exact: true });
  await section.getByRole('button', { name: 'Configure Host basics', exact: true }).click();
  const pluginPage = page.getByRole('region', { name: 'Host basics plugin', exact: true });
  const panel = pluginPage.getByRole('region', { name: 'Update Host basics', exact: true });
  async function check(url) {
    const form = panel.getByRole('form', { name: 'Update source for Host basics' });
    await form.getByRole('radio', { name: 'HTTPS address' }).check();
    await form.getByLabel('Plugin address', { exact: true }).fill(url);
    await form.getByRole('button', { name: 'Check update', exact: true }).click();
  }
  const shown = panel.getByRole('group', { name: 'What the update changes' });

  // 1. Nothing new: applies, keeps the review, logged.
  await check(address('same'));
  await expect(shown.getByRole('heading')).toHaveText('Version 1.0.0 → 1.0.1', { timeout: 30000 });
  await expect(shown).toContainText('This update asks for nothing new.');
  await expect(shown.getByRole('list', { name: 'Changed contributions' })).toHaveText('Title or presentation changed: Kernel release');
  await expect(shown).toContainText('Signature not checked: no public key was given.');
  // Nothing has changed yet: the installed version still answers.
  expect((await view()).version).toBe('1.0.0');
  await page.screenshot({ path: path.join(directory, 'update-same.png') });
  await shown.getByRole('button', { name: 'Apply update', exact: true }).click();
  await expect(section.getByRole('status')).toHaveText('Host basics updated to 1.0.1. It keeps working with the permissions you set.');
  let v = await view();
  expect([v.version, v.status, v.trust, v.grants[0].capabilities]).toEqual(['1.0.1', 'enabled', 'read_only', BOTH]);
  expect((await allowed('run_approved_command')).allowed).toBe(true);
  results.push('an update that only renamed a contribution was shown as "asks for nothing new" with the installed version still in place, then applied: version 1.0.1, still enabled read-only with the same permissions');

  // 2. A changed command: shown in full, applied, switched off until reviewed.
  await check(address('changed-command'));
  await expect(shown).toContainText('This update asks for something you have not reviewed.', { timeout: 30000 });
  await expect(shown.getByRole('list', { name: 'Changed contributions' })).toContainText('Command changed: Kernel version');
  await expect(shown.getByRole('list', { name: 'Changed contributions' })).toContainText('uname -a');
  await page.screenshot({ path: path.join(directory, 'update-review.png') });
  await shown.getByRole('button', { name: 'Apply and review again', exact: true }).click();
  await expect(section.getByRole('status')).toHaveText('Host basics updated to 1.0.1. It is switched off until you review and enable it again.');
  v = await view();
  expect(v.status).toBe('changed_since_review');
  expect([(await allowed('ssh_session')).allowed, (await allowed('ssh_session')).refusal]).toEqual([false, 'changed_since_review']);
  expect((await invoke('pack_state', { serverId: server.id })).commands).toEqual([]);
  await expect(pluginPage).toContainText('Status: Changed since you reviewed it · switched off');
  await pluginPage.getByRole('radio', { name: /^Read-only/ }).check();
  await pluginPage.getByRole('button', { name: 'Enable plugin', exact: true }).click();
  await expect(pluginPage).toContainText('Status: Enabled · Read-only');
  expect((await allowed('ssh_session')).allowed).toBe(true);
  results.push('an update that changed a command with the same capabilities showed "uname -a" and "asks for something you have not reviewed"; after applying, the plugin was switched off and refused everything until reviewed and enabled again, which restored exactly the earlier permissions');

  // 3. A new capability: never granted by the update.
  await check(address('added-capability'));
  await expect(shown).toContainText('Asks for more: loopback_tunnel', { timeout: 30000 });
  await expect(shown.getByRole('list', { name: 'Changed contributions' })).toContainText('New: Status page');
  await shown.getByRole('button', { name: 'Apply and review again', exact: true }).click();
  await expect(section.getByRole('status')).toContainText('updated to 2.0.0. It is switched off');
  expect((await view()).status).toBe('changed_since_review');
  await invoke('plugin_enable', { pluginId: PACK, trust: 'trusted_automation' });
  expect((await allowed('ssh_session')).allowed).toBe(true);
  expect((await allowed('loopback_tunnel')).refusal).toBe('not_granted');
  expect((await view()).grants[0].capabilities).toEqual(BOTH);
  results.push('an update asking for loopback_tunnel listed it under "Asks for more" and the new application; after applying, the plugin was off until enabled again, and even at trusted automation loopback_tunnel stayed not granted');

  // 4. A downgrade needs confirming.
  await check(address('downgrade'));
  await expect(shown.getByRole('heading')).toHaveText('Version 2.0.0 → 0.9.0', { timeout: 30000 });
  // It only removes what 2.0.0 added, so it asks for nothing new.
  await expect(shown).toContainText('This update asks for nothing new.');
  const apply = shown.getByRole('button', { name: 'Apply update', exact: true });
  await expect(apply).toBeDisabled();
  await shown.getByRole('checkbox', { name: 'I want to go back to the older version 0.9.0.' }).check();
  await apply.click();
  await expect(section.getByRole('status')).toContainText('updated to 0.9.0');
  expect((await view()).version).toBe('0.9.0');
  results.push('a downgrade to 0.9.0 (which only removed what 2.0.0 added, so it kept working) could be applied only after ticking the confirmation (the core refusing it otherwise is a core test)');

  // 5. Failed updates leave the previous version working.
  await invoke('plugin_enable', { pluginId: PACK, trust: 'read_only' });
  const before = await folderBytes();
  for (const [c, words] of [['invalid', 'The manifest is not valid.'], ['different-plugin', 'Another plugin.']]) {
    await check(address(c));
    await expect(panel.getByRole('alert')).toContainText(words, { timeout: 30000 });
    await expect(panel.getByRole('alert')).toContainText('The installed version is unchanged.');
  }
  // Checked, then the installed manifest changes before applying.
  let checked = await invoke('plugin_update_check', { pluginId: PACK, request: { source: { kind: 'folder', path: path.join(updates, 'same', 'v2') } } });
  expect((await invoke('plugin_update_apply', { token: checked.token, allowDowngrade: true })).diff.to_version).toBe('1.0.1');
  expect((await invoke('plugin_update_apply', { token: checked.token, allowDowngrade: true })).problem.code).toBe('preview_expired');
  const afterSame = await folderBytes();
  checked = await invoke('plugin_update_check', { pluginId: PACK, request: { source: { kind: 'folder', path: path.join(updates, 'changed-command', 'v2') } } });
  const manifest = path.join(plugins, PACK, 'infrawerf-plugin.json');
  const original = await readFile(manifest);
  await writeFile(manifest, original.toString().replace('"Example Author"', '"Someone Else"'));
  expect((await invoke('plugin_update_apply', { token: checked.token, allowDowngrade: false })).problem.code).toBe('changed_since_preview');
  await writeFile(manifest, original);
  expect(await folderBytes()).toEqual(afterSame);
  expect(before.length).toBe(afterSame.length);
  v = await view();
  expect([v.version, v.status]).toEqual(['1.0.1', 'enabled']);
  expect((await allowed('ssh_session')).allowed).toBe(true);
  expect((await readdir(plugins)).sort()).toEqual([PACK]);
  results.push('an invalid update and one with another plugin\'s ID were refused with "The installed version is unchanged"; an update whose installed plugin changed after checking was refused with changed_since_preview, and a token used twice with preview_expired; after each, the installed files were byte-for-byte unchanged and the plugin kept working');

  // 6. Fewer capabilities: applies, and the dropped permission goes.
  await invoke('plugin_remove', { folder: PACK });
  expect((await invoke('plugin_install', { request: { source: { kind: 'folder', path: path.join(updates, 'fewer', 'v1') } } })).problem).toBe(null);
  await invoke('plugin_enable', { pluginId: PACK, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: PACK, serverId: server.id, capabilities: [...BOTH, 'network_probe'] });
  checked = await invoke('plugin_update_check', { pluginId: PACK, request: { source: { kind: 'folder', path: path.join(updates, 'fewer', 'v2') } } });
  expect([checked.diff.needs_review, checked.diff.removed_capabilities]).toEqual([false, ['network_probe']]);
  await invoke('plugin_update_apply', { token: checked.token, allowDowngrade: false });
  v = await view();
  expect([v.status, v.grants[0].capabilities]).toEqual(['enabled', BOTH]);
  expect((await allowed('network_probe')).refusal).toBe('not_requested');
  results.push('an update asking for fewer capabilities applied without review, kept working, and removed the permission for the capability it dropped');

  // 7. Everything is in the activity log.
  const log = (await invoke('plugin_activity', { limit: 500 })).entries.filter(e => e.action === 'update').map(e => [e.outcome, e.detail]).reverse();
  expect(log).toEqual([
    ['done', '1.0.0 to 1.0.1'], ['done', '1.0.1 to 1.0.1, needs your review'], ['done', '1.0.1 to 2.0.0, needs your review'],
    ['done', '2.0.0 to 0.9.0, downgrade'], ['done', '0.9.0 to 1.0.1'], ['failed', 'changed_since_preview'], ['done', '1.0.0 to 1.1.0'],
  ]);
  const activity = page.getByRole('region', { name: 'Plugin activity', exact: true }).getByRole('table', { name: 'Plugin activity log' });
  await page.getByRole('region', { name: 'Plugin activity', exact: true }).getByRole('button', { name: 'Refresh activity' }).click();
  for (const text of ['Updated by you', 'Version 1.0.0 to 1.0.1', 'Not applied (changed_since_preview); the installed version is unchanged']) await expect(activity).toContainText(text);
  results.push('every applied and refused update is in the plugin activity log with its versions and whether it needed a review');

  expect(pageErrors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'loopback HTTPS server and local files only', noRealHost: true, pageErrors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native plugin update proof failed in ${directory}.`);
  throw failure;
} finally {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(resolve => app.once('exit', resolve)); }
  if (fixture.exitCode === null) { fixture.stdin.end(); await Promise.race([new Promise(resolve => fixture.once('exit', resolve)), delay(5000)]); if (fixture.exitCode === null) fixture.kill(); }
  lines.close();
}
