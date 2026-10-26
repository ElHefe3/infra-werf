// Actual Tauri/WebView2 health UI against disposable loopback HTTP/TCP targets.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { createServer as createHttpServer } from 'node:http';
import { createServer as createTcpServer } from 'node:net';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-health-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const digest = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const listen = server => new Promise((resolve, reject) => {
  server.once('error', reject);
  server.listen(0, '127.0.0.1', () => { server.off('error', reject); resolve(server.address().port); });
});
const close = server => new Promise(resolve => server?.close(() => resolve()));
let serviceStatus = 200;
const hostServer = createTcpServer(socket => socket.end());
const serviceServer = createHttpServer((_request, response) => {
  response.writeHead(serviceStatus, { connection: 'close' });
  response.end();
});
let app, browser, page;
const results = [];
try {
  const hostPort = await listen(hostServer), servicePort = await listen(serviceServer);
  const database = path.join(directory, 'native.sqlite3');
  const sshConfig = path.join(directory, 'ssh_config');
  const knownHosts = path.join(directory, 'known_hosts');
  await writeFile(sshConfig, `Host *\n  BatchMode yes\n  StrictHostKeyChecking yes\n  UserKnownHostsFile ${knownHosts}\n`);
  await writeFile(knownHosts, '');
  const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  app = spawn(path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root,
    windowsHide: true,
    env: {
      ...environment,
      ATLAS_NATIVE_DB: database,
      ATLAS_SSH_CONFIG: sshConfig,
      WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9235 --remote-debugging-address=127.0.0.1',
    },
    stdio: 'ignore',
  });
  for (let i = 0; i < 80; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9235/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9235');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(10000);
  await page.waitForLoadState('domcontentloaded');
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'health fixture', host: '127.0.0.1' } });
  await page.reload();
  await expect(page.locator('.server-card')).toHaveCount(1);
  const before = await invoke('map_state');
  const beforeDb = await digest(database);

  await page.getByRole('button', { name: 'Open health fixture workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'health fixture', exact: true });
  await workspace.getByRole('tab', { name: 'Health', exact: true }).click();
  const health = workspace.locator('.health-panel');
  await expect(health.locator('.health-results .state-pill')).toHaveCount(2);
  await expect(health.locator('.health-results .state-pill').nth(0)).toHaveText('unknown');
  await expect(health.locator('.health-results .state-pill').nth(1)).toHaveText('unknown');
  results.push('unconfigured observation displays distinct unknown host/service states');

  await health.locator('input[name="hostPort"]').fill(String(hostPort));
  await health.locator('select[name="kind"]').selectOption('http');
  await health.locator('input[name="servicePort"]').fill(String(servicePort));
  await health.locator('input[name="path"]').fill('/health');
  await health.locator('input[name="interval"]').fill('15');
  await health.getByRole('button', { name: 'Save & start checks', exact: true }).click();
  const states = health.locator('.health-results .state-pill');
  await expect.poll(() => states.nth(0).innerText(), { timeout: 15000 }).toBe('reachable');
  await expect.poll(() => states.nth(1).innerText(), { timeout: 15000 }).toBe('healthy');
  const hostResults = health.locator('.health-results > div').nth(0);
  const serviceResults = health.locator('.health-results > div').nth(1);
  const serviceHeading = serviceResults.locator('dt');
  await expect(serviceHeading).toHaveText(`Service · HTTP ${servicePort}`);
  const healthyHostTimes = await hostResults.innerText();
  const healthyServiceTimes = await serviceResults.innerText();
  expect(healthyHostTimes).toMatch(/Last checked · (?!No observation)[^\n]+/);
  expect(healthyHostTimes).toMatch(/Last success · (?!No observation)[^\n]+/);
  expect(healthyServiceTimes).toMatch(/Last checked · (?!No observation)[^\n]+/);
  expect(healthyServiceTimes).toMatch(/Last success · (?!No observation)[^\n]+/);
  const healthySnapshot = await invoke('health_status', { serverId: before.servers[0].id });
  expect(healthySnapshot.service.last_success_at).toBeTruthy();
  const healthyServiceSuccessLabel = healthyServiceTimes.match(/Last success · [^\n]+/)?.[0];
  expect(healthyServiceSuccessLabel).toBeTruthy();
  results.push('host TCP and HTTP service independently report healthy with timestamps');

  await health.locator('input[name="servicePort"]').fill(String(servicePort + 1));
  await expect(serviceHeading).toHaveText(`Service · HTTP ${servicePort}`);
  await health.locator('input[name="path"]').fill('invalid-path');
  await health.getByRole('button', { name: 'Save & start checks', exact: true }).click();
  await expect(health.getByRole('alert')).toBeVisible();
  await expect(serviceHeading).toHaveText(`Service · HTTP ${servicePort}`);
  await health.locator('input[name="servicePort"]').fill(String(servicePort));
  await health.locator('input[name="path"]').fill('/health');
  results.push('editing a draft or rejecting Save leaves active result endpoint labels unchanged');

  await page.clock.install();
  await page.clock.setFixedTime(new Date(Date.now() + 16 * 60 * 1000));
  await expect(health.getByText(/Observation · Stale/)).toBeVisible();
  await page.clock.setFixedTime(new Date());
  results.push('old observation changes to an explicit stale label without persisting health data');

  serviceStatus = 503;
  await expect.poll(() => states.nth(0).innerText(), { timeout: 20000 }).toBe('reachable');
  await expect.poll(() => states.nth(1).innerText(), { timeout: 20000 }).toBe('unhealthy');
  const failedServiceTimes = await serviceResults.innerText();
  const failedSnapshot = await invoke('health_status', { serverId: before.servers[0].id });
  expect(failedSnapshot.service.last_success_at).toBe(healthySnapshot.service.last_success_at);
  expect(failedServiceTimes.match(/Last success · [^\n]+/)?.[0]).toBe(healthyServiceSuccessLabel);
  expect(failedServiceTimes).toMatch(/Last failure · (?!No observation)[^\n]+/);
  results.push('reachable host remains reachable while the HTTP service transitions to unhealthy with last-failure time');

  await close(serviceServer);
  await health.getByRole('button', { name: 'Save & start checks', exact: true }).click();
  await expect.poll(() => states.nth(0).innerText(), { timeout: 15000 }).toBe('reachable');
  await expect.poll(() => states.nth(1).innerText(), { timeout: 15000 }).toBe('unhealthy');
  results.push('closed service endpoint remains distinct from the reachable host');

  await health.getByRole('button', { name: 'Stop checks', exact: true }).click();
  await expect(states.nth(0)).toHaveText('unknown');
  await expect(states.nth(1)).toHaveText('unknown');
  await expect(health.getByText(/Observation · No observation yet\./)).toBeVisible();
  await expect.poll(() => page.evaluate(() => Object.keys(localStorage).filter(key => key.startsWith('atlas.health.')))).toEqual([]);
  results.push('Stop checks cancels polling, clears the ephemeral observation and removes the saved probe configuration');

  const after = await invoke('map_state');
  expect(after.servers).toEqual(before.servers);
  expect(after.edges).toEqual(before.edges);
  expect(after.events).toEqual(before.events);
  expect(await digest(database)).toBe(beforeDb);
  expect(errors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'loopback only', healthObservationsPersisted: false, mapUnchanged: true, pageErrors: errors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Quit InfraWerf', exact: true }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native Health proof artifacts: ${directory}`);
  await page?.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
  throw failure;
} finally {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(resolve => app.once('exit', resolve)); }
  await Promise.all([close(hostServer), close(serviceServer)]);
}
