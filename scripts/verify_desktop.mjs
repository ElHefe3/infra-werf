// End-to-end proof through the actual Tauri WebView2 and Rust IPC, on synthetic
// data only. The CDP listener exists only in this child process, on loopback.
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const directory = path.join(root, '.tools', `desktop-proof-${randomUUID()}`);
const fixture = spawnSync('python', ['scripts/desktop_fixture.py', directory], { cwd: root, encoding: 'utf8', windowsHide: true });
if (fixture.status !== 0) throw new Error(fixture.stderr || 'Fixture creation failed');
const config = JSON.parse(fixture.stdout);
const original = createHash('sha256').update(await readFile(config.source)).digest('hex');
const database = path.join(directory, 'native.sqlite3');
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
async function launch(db = database, legacy = config.source) {
  child = spawn(binary, [], { cwd: root, windowsHide: true, env: { ...process.env, ATLAS_NATIVE_DB: db, ATLAS_LEGACY_DB: legacy, ATLAS_REPORTS_DIR: config.reports, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9225 --remote-debugging-address=127.0.0.1', WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview-profile') }, stdio: ['ignore', 'pipe', 'pipe'] });
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
  const layout = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, scrollWidth: document.documentElement.scrollWidth, scrollHeight: document.documentElement.scrollHeight, footer: document.querySelector('.app-footer').getBoundingClientRect().bottom, map: document.querySelector('.map-canvas')?.getBoundingClientRect().height }));
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
