// Actual native WebView2/Rust/OpenSSH/SFTP journey, fixture data only.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';
const root = process.cwd(), directory = path.join(root, '.tools', `native-files-proof-${randomUUID()}`);
await mkdir(directory);
const digest = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const server = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/ssh_fixture.py', path.join(directory, 'fixture')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
const lines = createInterface({ input: server.stdout });
let diagnostic = ''; server.stderr.on('data', data => { diagnostic += data.toString().slice(0, 1000); });
let app, browser, page; const results = [];
try {
  const info = await new Promise((resolve, reject) => { const timer = setTimeout(() => reject(new Error(`Fixture startup failed: ${diagnostic}`)), 30000); lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); }); server.once('exit', code => { if (code) reject(new Error(`Fixture exited: ${diagnostic}`)); }); });
  const originals = await Promise.all([info.config, info.known_hosts, info.fallback_known_hosts].map(digest));
  const database = path.join(directory, 'native.sqlite3');
  const { ATLAS_LEGACY_DB, ...environment } = process.env;
  app = spawn(path.join(root, 'target/debug/atlas-native.exe'), [], { cwd: root, windowsHide: true, env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_SSH_CONFIG: info.config, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9229 --remote-debugging-address=127.0.0.1' }, stdio: 'ignore' });
  for (let i = 0; i < 80; i++) { if (app.exitCode !== null) throw new Error('Native startup failed'); try { if ((await fetch('http://127.0.0.1:9229/json/version')).ok) break; } catch {} await delay(200); }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9229'); page = browser.contexts()[0].pages()[0];
  await page.waitForLoadState('domcontentloaded'); const errors = []; page.on('pageerror', error => errors.push(error.message));
  const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  const rejection = (command, args) => page.evaluate(async ({ command, args }) => { try { await window.__TAURI_INTERNALS__.invoke(command, args); return { unexpectedSuccess: true }; } catch (error) { return error; } }, { command, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  const aliases = ['trusted', 'unknown', 'changed', 'rejected', 'slow', 'overflow', 'no-auth'];
  for (const name of aliases) await invoke('add_server', { input: { name: `${name} fixture`, host: `atlas-${name}` } });
  await page.reload(); await expect(page.locator('.server-card')).toHaveCount(aliases.length);
  const before = await invoke('map_state'), beforeDb = await digest(database);
  let expectedMap = before, expectedDb = beforeDb;
  const stats = async () => JSON.parse(await readFile(info.stats, 'utf8'));
  async function files(name) {
    await page.getByRole('button', { name: `Open ${name} fixture workspace`, exact: true }).click();
    const dialog = page.getByRole('dialog', { name: `${name} fixture`, exact: true });
    await dialog.getByRole('tab', { name: 'Files', exact: true }).click(); return dialog;
  }
  let dialog = await files('trusted');
  const home = async () => { await dialog.getByRole('navigation', { name: 'Remote directory' }).getByRole('button', { name: 'fixture', exact: true }).click(); await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture'); };
  const open = async name => { await dialog.getByRole('button', { name: `Open folder ${name}`, exact: true }).click(); await expect(dialog.locator('.file-location code')).toHaveText(new RegExp(`/${name}$`)); };
  await dialog.getByRole('button', { name: 'Open file session', exact: true }).click();
  await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture');
  await expect(dialog.getByRole('button', { name: 'odd-\\xff-name', exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: ';$(echo injected).txt', exact: true }).click();
  await expect(dialog.getByRole('complementary', { name: 'Selected file' })).toContainText('9007199254740993');
  await open('space folder'); await dialog.getByRole('button', { name: '日本語.txt', exact: true }).click();
  await expect(dialog.getByRole('complementary', { name: 'Selected file' })).toContainText('Permissions'); await home();
  await dialog.getByRole('button', { name: 'Open folder denied', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('cannot access');
  await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture');
  await dialog.getByRole('button', { name: 'folder-link', exact: true }).click();
  await expect(dialog.getByRole('complementary', { name: 'Selected file' })).toContainText('symlink');
  await dialog.getByRole('button', { name: 'Follow link to folder', exact: true }).click();
  await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture/space folder'); await home();
  await dialog.getByRole('button', { name: 'broken-link', exact: true }).click();
  await dialog.getByRole('button', { name: 'Follow link to folder', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('no longer exists');
  await dialog.getByRole('navigation', { name: 'Remote directory' }).getByRole('button', { name: '/', exact: true }).click();
  await expect(dialog.locator('.file-location code')).toHaveText('/'); await open('home'); await open('fixture');
  await open('deep'); for (let i = 0; i < 18; i++) { const previous = await dialog.locator('.file-location code').innerText(); await dialog.getByRole('button', { name: 'Open folder next', exact: true }).click(); await expect(dialog.locator('.file-location code')).toHaveText(`${previous}/next`); }
  await home(); await open('large');
  const names = [];
  while (true) {
    const entries = await dialog.locator('.file-list tbody .text-button').allTextContents(); expect(entries.length).toBeLessThanOrEqual(100); names.push(...entries);
    const next = dialog.getByRole('button', { name: 'Next page', exact: true }); if (await next.isDisabled()) break;
    const first = entries[0]; await next.click();
    await expect.poll(() => dialog.locator('.file-list tbody .text-button').first().textContent().catch(() => null)).not.toBe(first);
  }
  expect(new Set(names).size).toBe(350); expect(names.length).toBe(350);
  await page.screenshot({ path: path.join(directory, 'native-files-large.png') }); await home();
  await page.screenshot({ path: path.join(directory, 'native-files-home.png') });
  results.push('native root/home/deep/350-entry paging, Unicode/spaces/non-UTF8, exact u64 stat, symlink identity/follow and denied paths');
  await invoke('plugin:window|close', { label: 'main' }); await expect(dialog.getByRole('alert')).toContainText('Lock open sessions'); expect(app.exitCode).toBe(null);
  await dialog.getByRole('button', { name: 'Lock session', exact: true }).click();
  await expect(dialog.locator('.file-list')).toHaveCount(0); await expect(dialog.locator('.file-session')).toHaveCount(0);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(0);
  await expect.poll(async () => (await stats()).trusted.active).toBe(0);
  await dialog.getByRole('button', { name: 'Open file session', exact: true }).click(); await expect(dialog.locator('.file-list')).toBeVisible();
  expect(await digest(database)).toBe(beforeDb);
  const fixtureId = before.servers.find(s => s.host === 'atlas-trusted').id;
  // A real backend host edit invalidates the visible session independently of
  // component Lock. Both edits are explicit and affect this private fixture DB.
  await invoke('update_server', { id: fixtureId, input: { host: 'atlas-fallback' } });
  await expect(dialog.locator('.file-list')).toHaveCount(0); await expect(dialog.locator('.file-session')).toHaveCount(0);
  await expect(dialog.getByRole('status')).toContainText('Protected file data has been cleared');
  await invoke('update_server', { id: fixtureId, input: { host: 'atlas-trusted' } });
  expectedMap = await invoke('map_state'); expectedDb = await digest(database);
  await dialog.getByRole('button', { name: 'Open file session', exact: true }).click(); await expect(dialog.locator('.file-list')).toBeVisible();
  await dialog.getByRole('tab', { name: 'Services', exact: true }).click(); await expect.poll(async () => (await stats()).trusted.active).toBe(0);
  await dialog.getByRole('tab', { name: 'Files', exact: true }).click(); await expect(dialog.locator('.file-list')).toHaveCount(0);
  results.push('native close guard, UI lock, backend server-edit invalidation and leaving Files clear protected data and reap SSH child');
  await page.keyboard.press('Escape');
  for (const [name, text] of [['unknown', 'host identity needs review'], ['changed', 'host identity needs review'], ['rejected', 'authentication failed'], ['no-auth', 'authentication failed'], ['overflow', 'safety limit']]) {
    dialog = await files(name); await dialog.getByRole('button', { name: 'Open file session', exact: true }).click();
    await expect(dialog.getByRole('alert')).toContainText(text); await expect(dialog.locator('.file-list')).toHaveCount(0);
    await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(0);
    await page.keyboard.press('Escape'); results.push(`${name}: protected data denied/cleared`);
  }
  dialog = await files('slow'); await dialog.getByRole('button', { name: 'Open file session', exact: true }).click();
  await expect.poll(async () => (await stats()).slow.subsystems).toBe(1);
  await dialog.getByRole('button', { name: 'Cancel connection', exact: true }).click();
  await expect(dialog.getByRole('status')).toContainText('cancelled'); await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(0);
  await expect.poll(async () => (await stats()).slow.active).toBe(0); await page.keyboard.press('Escape');
  results.push('cancel during actual SFTP bootstrap kills/reaps child and releases native lease');
  const trusted = before.servers.find(s => s.host === 'atlas-trusted'), changed = before.servers.find(s => s.host === 'atlas-changed');
  const ticket = await invoke('files_prepare', { serverId: trusted.id });
  const session = await invoke('files_open', { serverId: trusted.id, operationId: ticket.operation_id });
  const action = { kind: 'list', resource_id: session.home.id, cursor: null };
  expect((await rejection('files_read', { serverId: changed.id, sessionId: session.session_id, action })).code).toBe('denied');
  expect((await rejection('files_read', { serverId: trusted.id, sessionId: session.session_id, action: { ...action, resource_id: '/etc/passwd; echo injected' } })).code).toBe('denied');
  const extra = await rejection('files_read', { serverId: trusted.id, sessionId: session.session_id, action: { ...action, path: '/etc/passwd' } });
  expect(typeof extra === 'string' ? extra : extra.code).toMatch(/invalid|unknown|denied/i);
  expect((await rejection('files_open', { serverId: trusted.id, operationId: ticket.operation_id })).code).toBe('conflict');
  await invoke('files_lock', { sessionId: session.session_id });
  expect((await rejection('files_read', { serverId: trusted.id, sessionId: session.session_id, action })).code).toBe('expired_session');
  results.push('native wrong host, forged path/resource, extra fields, replayed ticket and locked handle denied');
  dialog = await files('trusted'); await dialog.getByRole('button', { name: 'Open file session', exact: true }).click(); await expect(dialog.locator('.file-list')).toBeVisible();
  await dialog.getByRole('button', { name: 'Open folder disconnect', exact: true }).click();
  await expect(dialog.locator('.file-list')).toHaveCount(0); await expect(dialog.locator('.file-session')).toHaveCount(0);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(0);
  await page.keyboard.press('Escape'); results.push('actual disconnect invalidates session and clears visible data');
  const after = await invoke('map_state'); expect(after.servers).toEqual(expectedMap.servers); expect(after.edges).toEqual(expectedMap.edges); expect(after.events).toEqual(expectedMap.events); expect(await digest(database)).toBe(expectedDb);
  expect(await Promise.all([info.config, info.known_hosts, info.fallback_known_hosts].map(digest))).toEqual(originals);
  const finalStats = await stats(); expect(finalStats.unknown.auth_attempts).toBe(0); expect(finalStats.changed.auth_attempts).toBe(0);
  for (const row of Object.values(finalStats)) { expect(row.commands).toBe(0); expect(row.mutations).toBe(0); }
  expect(errors).toEqual([]);
  const proof = { result: 'pass', results, readOnlyJourneysPreserveMap: true, explicitSyntheticMapEdits: 2, sshConfigTrustUnchanged: true, noShellOrFileWrites: true, pageErrors: errors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2)); console.log(JSON.stringify(proof, null, 2));
  await page.getByRole('button', { name: 'Quit Atlas', exact: true }).click(); await expect.poll(() => app.exitCode).not.toBe(null);
} catch (error) {
  await page?.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {}); console.error(`Native Files proof: ${directory}`); throw error;
} finally {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(resolve => app.once('exit', resolve)); }
  if (server.exitCode === null) { server.stdin.end('\n'); await Promise.race([new Promise(resolve => server.once('exit', resolve)), delay(5000)]); if (server.exitCode === null) server.kill(); }
  lines.close();
}
