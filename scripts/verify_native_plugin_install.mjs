// Actual Windows Tauri/WebView2 proof for installing, verifying and removing
// plugins (F22/T045): from an HTTPS address (a loopback server with a
// throwaway CA trusted only by this InfraWerf process), an archive file and a
// folder; malicious archives refused with nothing left; the files hash
// re-checked at start-up; removal clearing files, permissions and the record.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { appendFile, mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-plugin-install-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const plugins = path.join(directory, 'plugins');
await mkdir(plugins);
const archives = path.join(root, 'crates/atlas-core/tests/fixtures/plugin-archives');
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const folders = async () => (await readdir(plugins)).sort();
const PACK = 'org.example.host-basics', MAP = 'org.example.homelab-map';

const fixture = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/plugin_https_fixture.py', path.join(directory, 'https')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
const lines = createInterface({ input: fixture.stdout });
let diagnostic = '';
fixture.stderr.on('data', data => { diagnostic += data.toString().slice(0, 2000); });
const https = await new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(new Error(`HTTPS fixture startup failed: ${diagnostic}`)), 30000);
  lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); });
  fixture.once('exit', code => { clearTimeout(timer); reject(new Error(`HTTPS fixture exited ${code}: ${diagnostic}`)); });
});
const base = `https://127.0.0.1:${https.port}`;
const database = path.join(directory, 'native.sqlite3');
const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
let app, browser, page;
const pageErrors = [];
async function launch() {
  app = spawn(process.env.ATLAS_DESKTOP_BINARY ? path.resolve(process.env.ATLAS_DESKTOP_BINARY) : path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root, windowsHide: true,
    env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_PLUGINS_DIR: plugins, ATLAS_PLUGIN_TEST_CA: https.ca, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9240 --remote-debugging-address=127.0.0.1' },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9240/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9240');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  page.on('pageerror', error => pageErrors.push(error.message));
  // The workspace has loaded once the main menu is drawn.
  await expect(page.getByRole('button', { name: 'Quit InfraWerf' })).toBeVisible({ timeout: 30000 });
  await expect(page.locator('.loading-state')).toHaveCount(0, { timeout: 30000 });
}
async function quit() {
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode, { timeout: 15000 }).not.toBe(null);
  await browser.close().catch(() => {});
}
const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
const results = [];
try {
  await launch();
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'Install host', host: 'install-fixture.home' } });
  const server = (await invoke('map_state')).servers[0];
  const nav = page.getByRole('navigation', { name: 'Main navigation' });
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  const section = page.getByRole('region', { name: 'Plugins', exact: true });
  await expect(section).toContainText('No plugins found.');

  // 1. From an HTTPS address, signed, through the form.
  const form = section.getByRole('form', { name: 'Install a plugin' });
  await form.getByRole('radio', { name: 'HTTPS address' }).check();
  await form.getByLabel('Plugin address', { exact: true }).fill(`${base}/good.zip`);
  await form.getByText('Check it before installing (optional)').click();
  const goodZip = await readFile(path.join(archives, 'good.zip'));
  await form.getByLabel('Expected SHA-256', { exact: true }).fill(sha256(goodZip));
  const publicKey = (await readFile(path.join(archives, 'author.pub'), 'utf8')).trim().split('\n').at(-1);
  await form.getByLabel('Author public key', { exact: true }).fill(publicKey);
  await form.getByRole('button', { name: 'Install', exact: true }).click();
  await expect(section.getByRole('status')).toHaveText('Host basics installed. It is disabled: open its page to review and enable it.', { timeout: 30000 });
  await expect(section.getByRole('list', { name: 'Installed plugins' })).toContainText('Host basics');
  await expect(section.getByRole('list', { name: 'Installed plugins' })).toContainText('Not enabled · allowed on 0 servers');
  expect(await folders()).toEqual([PACK]);
  await section.getByRole('button', { name: 'Configure Host basics', exact: true }).click();
  const pagePack = page.getByRole('region', { name: 'Host basics plugin', exact: true });
  const record = pagePack.getByRole('definition').filter({ hasText: base });
  await expect(record).toContainText(`Downloaded from ${base}/good.zip`);
  await expect(pagePack.getByLabel('Archive SHA-256', { exact: true })).toHaveText(sha256(goodZip));
  await expect(pagePack.getByLabel('Signer key ID', { exact: true })).toHaveText('BAEDC8198EBE6C24');
  await expect(pagePack).toContainText('Files match.');
  let state = await invoke('plugins_state');
  const installedHash = state.plugins[0].install.content_sha256;
  await expect(pagePack.getByLabel('Files SHA-256', { exact: true })).toHaveText(installedHash);
  expect(state.plugins[0].status).toBe('disabled');
  await page.screenshot({ path: path.join(directory, 'installed-plugin.png') });
  results.push(`installed from ${base}/good.zip through the form with the expected SHA-256 and the author's public key; it arrived disabled with no permissions, and its page shows the address, both hashes, the verified key ID BAEDC8198EBE6C24 and "Files match"`);

  // 2. Remove from its page: files, record and history.
  await pagePack.getByRole('button', { name: 'Remove Host basics', exact: true }).click();
  await pagePack.getByRole('group', { name: 'Remove Host basics?' }).getByRole('button', { name: 'Remove permanently' }).click();
  await expect(section.getByRole('status')).toHaveText('Host basics removed.');
  await expect(section).toContainText('No plugins found.');
  expect(await folders()).toEqual([]);
  results.push('removing it from its page after confirming deleted its folder and its record');

  // 3. From an archive file and from a folder.
  let report = await invoke('plugin_install', { request: { source: { kind: 'archive', path: path.join(archives, 'good-folder.tar.gz') } } });
  expect(report.problem).toBe(null);
  report = await invoke('plugin_install', { request: { source: { kind: 'folder', path: path.join(root, 'examples/homelab-map') } } });
  expect(report.problem).toBe(null);
  expect(await folders()).toEqual([MAP, PACK]);
  state = await invoke('plugins_state');
  expect(state.plugins.map(p => [p.id, p.status, p.install.source_kind, p.grants.length])).toEqual([[MAP, 'disabled', 'folder', 0], [PACK, 'disabled', 'archive', 0]]);
  results.push('installed again from a .tar.gz file and the homelab-map example from a folder; both disabled with no permissions');

  // 4. Malicious and mismatched sources are refused and leave nothing.
  const refused = [];
  for (const name of (await readdir(archives)).filter(n => n.startsWith('bad-') && !n.endsWith('.minisig')).sort()) {
    const code = name.slice(4).split(/[-.]/)[0];
    const request = { source: { kind: 'archive', path: path.join(archives, name) }, ...(code === 'signature_invalid' ? { public_key: publicKey } : {}) };
    const result = await invoke('plugin_install', { request });
    expect(result.problem?.code, name).toBe(code);
    refused.push(name);
  }
  expect(refused.length).toBe(16);
  const address = async (url, extra = {}) => (await invoke('plugin_install', { request: { source: { kind: 'address', url }, ...extra } })).problem?.code;
  expect(await address(`${base}/redirect-to-http.zip`)).toBe('unreadable');
  expect(await address(`${base}/big.zip`)).toBe('too_large');
  expect(await address(`http://127.0.0.1:${https.port}/good.zip`)).toBe('not_https');
  expect(await address(`${base}/missing.zip`)).toBe('unreadable');
  expect(await address(`${base}/good.zip`, { expected_sha256: '0'.repeat(64) })).toBe('hash_mismatch');
  expect(await address(`${base}/bad-signature_invalid-tampered.zip`, { public_key: publicKey })).toBe('signature_invalid');
  expect(await folders()).toEqual([MAP, PACK]);
  await section.getByRole('button', { name: 'Look again' }).click();
  const form2 = section.getByRole('form', { name: 'Install a plugin' });
  await form2.getByRole('radio', { name: 'HTTPS address' }).check();
  await form2.getByLabel('Plugin address', { exact: true }).fill(`${base}/bad-path_traversal.zip`);
  await form2.getByRole('button', { name: 'Install', exact: true }).click();
  await expect(form2.getByRole('alert')).toContainText('A path would leave the plugin\'s folder.', { timeout: 30000 });
  await expect(form2.getByRole('alert')).toContainText('Nothing was installed.');
  results.push(`all 16 malicious or broken archives (path traversal by .., backslash, absolute path, drive and duplicate name; symlinks and hard links in tar and zip; zip and tar.gz bombs; 300 files; no manifest; two top folders; not an archive; an invalid manifest; a tampered signed archive) were refused with their own codes, as were a redirect to http, a 6 MiB download, an http address, a missing address and a wrong expected hash; the plugins folder held only the two installed plugins afterwards`);

  // 5. The files hash is checked at start-up.
  await invoke('plugin_enable', { pluginId: MAP, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: MAP, serverId: server.id, capabilities: ['read_saved_state'] });
  expect((await invoke('plugin_authorize', { pluginId: MAP, serverId: server.id, capability: 'read_saved_state' })).allowed).toBe(true);
  await quit();
  // Restart unchanged: still verified and allowed.
  await launch();
  expect((await invoke('plugins_state')).plugins.find(p => p.id === MAP).install.integrity).toBe('verified');
  expect((await invoke('plugin_authorize', { pluginId: MAP, serverId: server.id, capability: 'read_saved_state' })).allowed).toBe(true);
  await quit();
  // A file other than the manifest changes while InfraWerf is closed.
  await appendFile(path.join(plugins, MAP, 'README.md'), '\nChanged after install.\n');
  await launch();
  state = await invoke('plugins_state');
  const changed = state.plugins.find(p => p.id === MAP);
  expect(changed.status).toBe('files_changed');
  expect(changed.install.integrity).toBe('changed');
  expect(state.plugins.find(p => p.id === PACK).install.integrity).toBe('verified');
  const decision = await invoke('plugin_authorize', { pluginId: MAP, serverId: server.id, capability: 'read_saved_state' });
  expect([decision.allowed, decision.refusal]).toEqual([false, 'files_changed']);
  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('button', { name: 'Settings', exact: true }).click();
  const section2 = page.getByRole('region', { name: 'Plugins', exact: true });
  await expect(section2.getByRole('list', { name: 'Installed plugins' })).toContainText('Files changed since it was installed · switched off');
  await section2.getByRole('button', { name: 'Configure Homelab map', exact: true }).click();
  const pageMap = page.getByRole('region', { name: 'Homelab map plugin', exact: true });
  await expect(pageMap).toContainText('Files changed since it was installed. Nothing it asks for is allowed. Remove it and install it again.');
  await expect(pageMap.getByRole('radio', { name: /^Read-only/ })).toHaveCount(0);
  await page.screenshot({ path: path.join(directory, 'files-changed.png') });
  results.push('after a restart with unchanged files the plugin stayed verified and allowed; after README.md was changed while InfraWerf was closed, the next start-up marked it "Files changed since it was installed", switched it off, refused its permission with files_changed and offered no way to enable it; the other plugin stayed verified');

  // 6. Removal clears files, permissions and the record; reinstalling starts from nothing.
  await pageMap.getByRole('button', { name: 'Remove Homelab map', exact: true }).click();
  await pageMap.getByRole('group', { name: 'Remove Homelab map?' }).getByRole('button', { name: 'Remove permanently' }).click();
  await expect(section2.getByRole('status')).toHaveText('Homelab map removed.');
  expect(await folders()).toEqual([PACK]);
  report = await invoke('plugin_install', { request: { source: { kind: 'folder', path: path.join(root, 'examples/homelab-map') } } });
  expect(report.problem).toBe(null);
  const again = (await invoke('plugins_state')).plugins.find(p => p.id === MAP);
  expect([again.status, again.grants, again.trust, again.install.integrity]).toEqual(['disabled', [], null, 'verified']);
  expect((await invoke('plugin_authorize', { pluginId: MAP, serverId: server.id, capability: 'read_saved_state' })).refusal).toBe('plugin_disabled');
  const log = (await invoke('plugin_activity', { limit: 500 })).entries;
  const owner = log.filter(e => ['install', 'remove'].includes(e.action)).map(e => [e.action, e.plugin_id, e.detail]).reverse();
  expect(owner).toEqual([['install', PACK, 'address'], ['remove', PACK, ''], ['install', PACK, 'archive'], ['install', MAP, 'folder'], ['remove', MAP, ''], ['install', MAP, 'folder']]);
  // History from before the removal is kept.
  expect(log.some(e => e.plugin_id === MAP && e.action === 'enable')).toBe(true);
  const activity = page.getByRole('region', { name: 'Plugin activity', exact: true }).getByRole('table', { name: 'Plugin activity log' });
  for (const text of ['Installed by you', 'From an HTTPS address · disabled until you enable it', 'Removed by you, with its permissions']) await expect(activity).toContainText(text);
  results.push('removing the changed plugin deleted its folder, trust, permissions and record; installing it again gave a disabled plugin with no permissions and a verified hash; the activity log shows every install and removal and keeps the earlier history');

  // The HTTPS server saw only the addresses the test asked for.
  expect(pageErrors).toEqual([]);
  await quit();
  fixture.stdin.end();
  await new Promise(resolve => fixture.once('exit', resolve));
  const seen = JSON.parse(await readFile(path.join(directory, 'https/requests.json'), 'utf8'));
  expect([...new Set(seen)].sort()).toEqual(['/bad-path_traversal.zip', '/bad-signature_invalid-tampered.zip', '/bad-signature_invalid-tampered.zip.minisig', '/big.zip', '/good.zip', '/good.zip.minisig', '/missing.zip', '/redirect-to-http.zip']);
  results.push('the HTTPS server received only the addresses the owner gave (and their .minisig when a key was given); the redirect to http was not followed');
  const proof = { result: 'pass', results, targets: 'loopback HTTPS server and local files only', noRealHost: true, pageErrors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
} catch (failure) {
  console.error(`Native plugin install proof failed in ${directory}.`);
  throw failure;
} finally {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(resolve => app.once('exit', resolve)); }
  if (fixture.exitCode === null) { fixture.stdin.end(); await Promise.race([new Promise(resolve => fixture.once('exit', resolve)), delay(5000)]); if (fixture.exitCode === null) fixture.kill(); }
  lines.close();
}
