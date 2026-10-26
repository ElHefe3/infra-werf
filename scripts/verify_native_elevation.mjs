// Actual Tauri/WebView2 + Rust + OpenSSH/sudo elevation flow on fixture data.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-elevation-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const digest = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const server = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/linux_fixture.py', path.join(directory, 'fixture')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
let diagnostic = '';
server.stderr.on('data', data => { diagnostic += data.toString().slice(0, 1000); });
const lines = createInterface({ input: server.stdout });
let app, browser, page;
let typedFixturePassword = false;
const results = [];
try {
  const info = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`SSH fixture startup failed: ${diagnostic}`)), 30000);
    lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); });
    server.once('exit', code => { if (code) { clearTimeout(timer); reject(new Error(`SSH fixture exited: ${diagnostic}`)); } });
  });
  const beforeConfig = await digest(info.config), beforeTrust = await digest(info.known_hosts);
  const database = path.join(directory, 'native.sqlite3');
  const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  app = spawn(path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root,
    windowsHide: true,
    env: {
      ...environment,
      ATLAS_NATIVE_DB: database,
      ATLAS_SSH_CONFIG: info.config,
      WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9231 --remote-debugging-address=127.0.0.1',
    },
    stdio: 'ignore',
  });
  for (let i = 0; i < 80; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9231/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9231');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(10000);
  await page.waitForLoadState('domcontentloaded');
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'elevation fixture', host: 'atlas-linux' } });
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(1);
  const before = await invoke('map_state');
  const beforeDb = await digest(database);

  await page.getByRole('button', { name: 'Open elevation fixture workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'elevation fixture', exact: true });
  await workspace.getByRole('tab', { name: 'Settings', exact: true }).click();
  const elevation = workspace.getByRole('region', { name: 'Fresh elevation check', exact: true });
  await elevation.getByRole('button', { name: 'Open protected session', exact: true }).click();
  await expect(elevation.locator('.file-session')).toContainText('SSH authenticated');
  await expect(elevation.getByRole('button', { name: 'Lock protected session', exact: true })).toBeVisible();
  expect(await elevation.locator('input[type="password"]').count()).toBe(0);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(1);
  results.push('native protected session opens with fixture SSH identity; no password field before review');

  await elevation.getByRole('button', { name: 'Review elevated access check', exact: true }).click();
  let review = elevation.getByRole('form', { name: 'Review elevated action', exact: true });
  await expect(review).toContainText('Check root identity without changing files or services');
  await review.getByRole('button', { name: 'Cancel elevation review', exact: true }).click();
  await expect(elevation.getByRole('form', { name: 'Review elevated action' })).toHaveCount(0);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(1);
  results.push('native review cancellation clears the preview before any credential is entered');

  const wrongPassword = `wrong-fixture-${randomUUID()}`;
  await elevation.getByRole('button', { name: 'Review elevated access check', exact: true }).click();
  review = elevation.getByRole('form', { name: 'Review elevated action', exact: true });
  await review.getByRole('textbox', { name: 'Password for this action', exact: true }).fill(wrongPassword);
  typedFixturePassword = true;
  await review.getByRole('button', { name: 'Confirm with fresh sudo', exact: true }).click();
  await expect(elevation.getByRole('alert')).toContainText('fresh password check failed');
  await expect(elevation.getByRole('form', { name: 'Review elevated action' })).toHaveCount(0);
  expect(await elevation.locator('input[type="password"]').count()).toBe(0);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(1);
  results.push('wrong password fails in the native flow; the password field and one-use review are cleared');

  await elevation.getByRole('button', { name: 'Review elevated access check', exact: true }).click();
  review = elevation.getByRole('form', { name: 'Review elevated action', exact: true });
  await expect(review).toContainText('atlas-linux');
  await review.getByRole('textbox', { name: 'Password for this action', exact: true }).fill(info.password);
  await review.getByRole('button', { name: 'Confirm with fresh sudo', exact: true }).click();
  await expect(elevation.getByRole('status')).toContainText('Fresh sudo check verified');
  await expect(elevation.getByRole('form', { name: 'Review elevated action' })).toHaveCount(0);
  expect(await elevation.locator('input[type="password"]').count()).toBe(0);
  await expect(elevation.getByRole('button', { name: 'Lock protected session', exact: true })).toBeVisible();
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(1);
  typedFixturePassword = false;
  const persistent = await readFile(database);
  expect(persistent.includes(Buffer.from(info.password))).toBe(false);
  expect(persistent.includes(Buffer.from(wrongPassword))).toBe(false);
  expect(await elevation.innerText()).not.toContain(info.password);
  expect(await elevation.innerText()).not.toContain(wrongPassword);
  await page.screenshot({ path: path.join(directory, 'native-elevation-verified.png') });
  results.push('native correct password verifies fresh sudo and root identity; field, UI text, and persistent database contain no fixture passwords');

  // A second reviewed action must prompt independently; sudo's prior timestamp
  // cannot make the wrong password succeed after the first success.
  await elevation.getByRole('button', { name: 'Review elevated access check', exact: true }).click();
  review = elevation.getByRole('form', { name: 'Review elevated action', exact: true });
  await review.getByRole('textbox', { name: 'Password for this action', exact: true }).fill(wrongPassword);
  typedFixturePassword = true;
  await review.getByRole('button', { name: 'Confirm with fresh sudo', exact: true }).click();
  await expect(elevation.getByRole('alert')).toContainText('fresh password check failed');
  await expect(elevation.getByRole('form', { name: 'Review elevated action' })).toHaveCount(0);
  expect(await elevation.locator('input[type="password"]').count()).toBe(0);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(1);
  typedFixturePassword = false;
  results.push('a later action prompts again and rejects the previous action’s wrong password');

  await elevation.getByRole('button', { name: 'Lock protected session', exact: true }).click();
  await expect(elevation.locator('.file-session')).toHaveCount(0);
  await expect(elevation.getByRole('button', { name: 'Open protected session', exact: true })).toBeVisible();
  await expect(elevation.getByRole('status')).toContainText('Session locked');
  expect(await elevation.locator('input[type="password"]').count()).toBe(0);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(0);
  results.push('native lock clears the authenticated session, protected display, pending work, and password control');
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await expect(workspace).toHaveCount(0);

  expect((await invoke('map_state')).servers).toEqual(before.servers);
  expect((await invoke('map_state')).events).toEqual(before.events);
  expect(await digest(database)).toBe(beforeDb);
  expect(await digest(info.config)).toBe(beforeConfig);
  expect(await digest(info.known_hosts)).toBe(beforeTrust);
  expect(errors).toEqual([]);
  const proof = { result: 'pass', results, fixtureOnly: true, noRealHost: true, mapDatabaseUnchangedDuringActions: true, sshConfigAndTrustUnchanged: true, pageErrors: errors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await page.getByRole('button', { name: 'Quit Atlas', exact: true }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  // Never persist a screenshot while the synthetic password may still be in
  // the field. The fixture password itself is never included in diagnostics.
  if (!typedFixturePassword) await page?.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
  console.error(`Native elevation proof artifacts: ${directory}`);
  throw failure;
} finally {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(resolve => app.once('exit', resolve)); }
  if (server.exitCode === null) {
    server.stdin.end('\n');
    await Promise.race([new Promise(resolve => server.once('exit', resolve)), delay(5000)]);
    if (server.exitCode === null) server.kill();
  }
  lines.close();
}
