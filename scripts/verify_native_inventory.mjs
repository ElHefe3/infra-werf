// Actual Tauri/WebView2 approved inventory refresh against a disposable SSH fixture.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-inventory-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const digest = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const server = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/linux_fixture.py', path.join(directory, 'fixture')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
const lines = createInterface({ input: server.stdout });
let diagnostic = '';
server.stderr.on('data', data => { diagnostic += data.toString().slice(0, 1000); });
let app, browser, page;
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
  app = spawn(process.env.ATLAS_DESKTOP_BINARY ? path.resolve(process.env.ATLAS_DESKTOP_BINARY) : path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root,
    windowsHide: true,
    env: {
      ...environment,
      ATLAS_NATIVE_DB: database,
      ATLAS_SSH_CONFIG: info.config,
      WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9232 --remote-debugging-address=127.0.0.1',
    },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9232/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9232');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();

  await invoke('add_server', { input: { name: 'Inventory fixture', host: 'atlas-linux' } });
  const firstState = await invoke('map_state');
  const serverId = firstState.servers[0].id;
  await invoke('add_server', { input: { name: 'Unaffected owner host', host: 'other-fixture.home' } });
  await invoke('update_server', { id: serverId, input: { name: 'Owner edited inventory host', host: 'atlas-linux', x: 321, y: 654 } });
  const otherId = (await invoke('map_state')).servers.find(server => server.name === 'Unaffected owner host').id;
  await invoke('add_relationship', { input: { source: serverId, target: otherId, label: 'owner documented link' } });
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(2);
  const before = await invoke('map_state');
  const beforeDb = await digest(database);
  const ownerBefore = before.servers.find(server => server.id === serverId);
  expect(ownerBefore.inventory).toBeNull();

  await page.getByRole('button', { name: 'Open Owner edited inventory host workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Owner edited inventory host', exact: true });
  const refresh = workspace.getByRole('button', { name: 'Refresh inventory', exact: true });
  await refresh.click();
  await expect(workspace.getByRole('status')).toContainText('Approved inventory updated');
  await expect(workspace).toContainText('Source collection:');
  await expect(workspace).toContainText('InfraWerf received:');
  await expect(workspace).toContainText('atlas-fixture.service');
  await expect(workspace).toContainText('fixture-worker.service');
  await expect(workspace).toContainText('atlas-fixture-api');
  results.push('explicit native refresh authenticates with the fixture identity and saves only the approved service/container baseline');

  const afterComplete = await invoke('map_state');
  const ownerAfter = afterComplete.servers.find(server => server.id === serverId);
  const completeDbHash = await digest(database);
  expect(ownerAfter.inventory).toMatchObject({ hostname: expect.any(String), cpus: expect.any(Number), memory_total_kib: expect.any(Number) });
  expect(ownerAfter.name).toBe(ownerBefore.name);
  expect(ownerAfter.host).toBe(ownerBefore.host);
  expect(ownerAfter.x).toBe(ownerBefore.x);
  expect(ownerAfter.y).toBe(ownerBefore.y);
  expect(ownerAfter.created_at).toBe(ownerBefore.created_at);
  expect(afterComplete.edges).toEqual(before.edges);
  expect(afterComplete.servers.find(server => server.id === otherId)).toEqual(before.servers.find(server => server.id === otherId));
  expect(afterComplete.events).toHaveLength(before.events.length + 1);
  expect(afterComplete.events[0].message).toMatch(/Initial approved inventory collected|Approved inventory refreshed/);
  const formattedSourceTime = await page.evaluate(value => new Date(value).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' }), ownerAfter.inventory.collected_at);
  expect(await workspace.getByRole('status').innerText()).toContain(formattedSourceTime);
  const completeDbBytes = await readFile(database);
  expect(completeDbBytes.includes(Buffer.from('Synthetic Atlas fixture service'))).toBe(false);
  results.push('stable IDs, owner name/host/layout, relationship and unrelated host survive the inventory update; excluded service descriptions are absent from SQLite');

  const partialReady = new Promise(resolve => lines.once('line', line => { if (line === 'PARTIAL_READY') resolve(); }));
  server.stdin.write('partial\n');
  await partialReady;
  await refresh.click();
  await expect(workspace.getByRole('status')).toContainText('Collection was partial; the saved snapshot is unchanged');
  await expect(workspace).toContainText('atlas-fixture-api');
  const afterPartial = await invoke('map_state');
  expect(afterPartial.servers.find(server => server.id === serverId).inventory).toEqual(ownerAfter.inventory);
  expect(afterPartial.events).toEqual(afterComplete.events);
  expect(await digest(database)).toBe(completeDbHash);
  expect(await digest(info.config)).toBe(beforeConfig);
  expect(await digest(info.known_hosts)).toBe(beforeTrust);
  results.push('partial collection is explicit and leaves the last approved inventory, source timestamp and event history unchanged');

  const authFailureReady = new Promise(resolve => lines.once('line', line => { if (line === 'AUTH_FAILURE_READY') resolve(); }));
  server.stdin.write('authfail\n');
  await authFailureReady;
  await refresh.click();
  await expect(workspace.getByRole('alert')).toContainText('Inventory refresh failed. The last saved observation was retained.');
  const afterAuthFailure = await invoke('map_state');
  expect(afterAuthFailure.servers.find(server => server.id === serverId).inventory).toEqual(ownerAfter.inventory);
  expect(afterAuthFailure.events).toEqual(afterComplete.events);
  expect(await digest(database)).toBe(completeDbHash);
  expect(await digest(info.config)).toBe(beforeConfig);
  expect(await digest(info.known_hosts)).toBe(beforeTrust);
  results.push('SSH authentication failure is visible and leaves the approved inventory, source timestamp, event history, SSH config and trust unchanged');

  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  expect(errors).toEqual([]);
  await writeFile(path.join(directory, 'result.json'), JSON.stringify({ result: 'pass', results, targets: 'disposable loopback SSH fixture only', noSudo: true, noRealHost: true, sshConfigAndTrustUnchanged: true, pageErrors: errors, directory }, null, 2));
  console.log(JSON.stringify({ result: 'pass', results, targets: 'disposable loopback SSH fixture only', noSudo: true, noRealHost: true, sshConfigAndTrustUnchanged: true, pageErrors: errors, directory }, null, 2));
  await page.getByRole('button', { name: 'Quit InfraWerf', exact: true }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native inventory proof artifacts: ${directory}`);
  await page?.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
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
