// Actual Windows Tauri/WebView2 protected workload inspection against a disposable SSH fixture.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-workload-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const fileHash = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const databaseSnapshot = async file => {
  const entries = [];
  for (const suffix of ['', '-wal', '-shm']) {
    try { entries.push([suffix, await fileHash(file + suffix)]); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return entries;
};
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
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9234 --remote-debugging-address=127.0.0.1',
    },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9234/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9234');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();

  await invoke('add_server', { input: { name: 'Workload fixture', host: 'atlas-linux' } });
  const initial = await invoke('map_state');
  const serverId = initial.servers[0].id;
  await invoke('add_server', { input: { name: 'Wrong target fixture', host: 'other-fixture.home' } });
  const otherId = (await invoke('map_state')).servers.find(server => server.name === 'Wrong target fixture').id;
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(2);
  await page.getByRole('button', { name: 'Open Workload fixture workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Workload fixture', exact: true });
  await workspace.getByRole('button', { name: 'Refresh inventory', exact: true }).click();
  await expect(workspace.getByRole('status')).toContainText('Approved inventory updated');
  await expect(workspace).toContainText('atlas-fixture-api');
  await expect(workspace).toContainText('atlas-fixture-db');

  const panel = workspace.locator('.workload-inspection');
  await expect(panel.getByRole('button', { name: 'Open protected view', exact: true })).toBeEnabled();
  await panel.getByRole('button', { name: 'Open protected view', exact: true }).click();
  await expect(panel.locator('tbody tr')).toHaveCount(10);
  const inspect = async (name, expectedState, expectedLog) => {
    const row = panel.locator('tbody tr').filter({ hasText: name });
    await row.getByRole('button', { name: 'Inspect', exact: true }).click();
    const result = panel.getByRole('region', { name: 'Protected workload result' });
    await expect(result).toContainText(expectedState);
    if (expectedLog) await expect(result).toContainText(expectedLog);
    results.push(`${name}: scoped inspection returned ${expectedState.toLowerCase()} with an explicit on-demand log result`);
    return result;
  };

  await inspect('atlas-fixture.service', 'Running', 'ATLAS_TEST_SYSTEM_SERVICE_LOG');
  await inspect('fixture-stopped.service', 'Stopped', 'ATLAS_TEST_STOPPED_SERVICE_LOG');
  await inspect('fixture-worker.service', 'Stopped', 'ATLAS_TEST_USER_SERVICE_LOG');
  await inspect('atlas-fixture-api', 'Running', 'ATLAS_TEST_PODMAN_CONTAINER_LOG');
  await inspect('atlas-fixture-db', 'Stopped', 'ATLAS_TEST_DOCKER_CONTAINER_LOG');
  await inspect('fixture-denied.service', 'Permission denied', 'Log access denied');
  await inspect('fixture-missing.service', 'Unavailable', 'Logs unavailable');
  await inspect('fixture-log-denied.service', 'Running', 'Log access denied');
  results.push('system and user systemd, user Podman, and system Docker scopes selected their matching adapters');

  const large = await inspect('fixture-large.service', 'Running', 'Older output trimmed to the 8 KiB limit');
  const bounded = await large.locator('.workload-logs').evaluate(element => ({
    bytes: new TextEncoder().encode(element.textContent ?? '').length,
    tailPresent: (element.textContent ?? '').includes('ATLAS_TEST_WORKLOAD_LOG_TAIL'),
  }));
  expect(bounded.bytes).toBeLessThanOrEqual(8192);
  expect(bounded.tailPresent).toBe(true);
  results.push('large logs are limited to 8 KiB and identify trimmed output');

  const beforeProtectedRead = await databaseSnapshot(database);
  const beforeMap = await invoke('map_state');
  const wrongIdentity = await page.evaluate(async ({ serverId, otherId }) => {
    const invoke = (command, args) => window.__TAURI_INTERNALS__.invoke(command, args);
    const sessionTicket = await invoke('files_prepare', { serverId });
    const session = await invoke('files_open', { serverId, operationId: sessionTicket.operation_id });
    let unknownDenied = false, wrongTargetDenied = false;
    try {
      await invoke('workload_inspect', { serverId, sessionId: session.session_id, workloadId: `wl1_${'a'.repeat(64)}` });
    } catch (error) { unknownDenied = String(error?.code ?? error).toLowerCase().includes('denied'); }
    try {
      await invoke('workload_inspect', { serverId: otherId, sessionId: session.session_id, workloadId: session.workloads[0].id });
    } catch (error) { wrongTargetDenied = String(error?.code ?? error).toLowerCase().includes('denied'); }
    await invoke('files_lock', { sessionId: session.session_id });
    return { unknownDenied, wrongTargetDenied };
  }, { serverId, otherId });
  expect(wrongIdentity).toEqual({ unknownDenied: true, wrongTargetDenied: true });
  results.push('unknown workload IDs and a session submitted against another host are denied before remote execution');

  await panel.getByRole('button', { name: 'Lock protected view', exact: true }).click();
  await expect(panel.getByRole('region', { name: 'Protected workload result' })).toHaveCount(0);
  expect(await databaseSnapshot(database)).toEqual(beforeProtectedRead);
  const afterMap = await invoke('map_state');
  expect(afterMap.servers).toEqual(beforeMap.servers);
  expect(afterMap.events).toEqual(beforeMap.events);
  const dbBytes = await readFile(database);
  for (const marker of [
    'ATLAS_TEST_SYSTEM_SERVICE_LOG', 'ATLAS_TEST_STOPPED_SERVICE_LOG', 'ATLAS_TEST_USER_SERVICE_LOG',
    'ATLAS_TEST_PODMAN_CONTAINER_LOG', 'ATLAS_TEST_DOCKER_CONTAINER_LOG', 'ATLAS_TEST_WORKLOAD_LOG_TAIL',
  ]) expect(dbBytes.includes(Buffer.from(marker))).toBe(false);
  results.push('Lock clears visible protected output; the database, overview inventory, and activity history contain no log markers');

  const originalConfig = await readFile(info.config, 'utf8');
  const normalizedConfig = originalConfig.replaceAll('\r\n', '\n');
  const originalAlias = `Host atlas-linux\n  Hostname 127.0.0.1\n  Port ${info.port}\n  User fixture\n`;
  const changedAlias = `Host atlas-linux\n  Hostname 127.0.0.1\n  Port ${info.port}\n  User fixture-none\n`;
  if (!normalizedConfig.includes(originalAlias)) throw new Error('Fixture SSH profile block was not found');
  try {
    await writeFile(info.config, normalizedConfig.replace(originalAlias, changedAlias), 'utf8');
    await panel.getByRole('button', { name: 'Open protected view', exact: true }).click();
    await expect(panel).toContainText('different SSH connection');
    await expect(panel.locator('tbody tr')).toHaveCount(0);
    await panel.getByRole('button', { name: 'Lock protected view', exact: true }).click();
    expect(await databaseSnapshot(database)).toEqual(beforeProtectedRead);
    results.push('changing the SSH account while keeping the alias and host key does not issue old workload IDs; the UI requests a fresh inventory');
  } finally {
    await writeFile(info.config, originalConfig, 'utf8');
  }

  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  expect(pageErrors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'disposable loopback SSH fixture only', noSudo: true, noRealHost: true, noPersistedLogs: true, pageErrors, directory };
  const proofPath = path.join(directory, 'result.json');
  await import('node:fs/promises').then(({ writeFile }) => writeFile(proofPath, JSON.stringify(proof, null, 2)));
  const proofBytes = await readFile(proofPath);
  for (const marker of ['ATLAS_TEST_SYSTEM_SERVICE_LOG', 'ATLAS_TEST_DOCKER_CONTAINER_LOG']) expect(proofBytes.includes(Buffer.from(marker))).toBe(false);
  console.log(JSON.stringify(proof, null, 2));
  await page.getByRole('button', { name: 'Quit Atlas' }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native workload proof failed in ${directory}; protected log contents were not captured as artifacts.`);
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
