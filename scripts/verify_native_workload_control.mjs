// Windows Tauri/WebView2 workload-control proof against a disposable loopback SSH fixture.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-workload-control-proof-${randomUUID()}`);
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
    const timer = setTimeout(() => reject(new Error(`SSH fixture startup failed: ${diagnostic}`)), 90000);
    lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); });
    server.once('exit', code => { if (code) { clearTimeout(timer); reject(new Error(`SSH fixture exited: ${diagnostic}`)); } });
  });
  const stats = async () => new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`fixture statistics timed out: ${diagnostic}`)), 30000);
    lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); });
    server.stdin.write('stats\n');
  });
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
  page.setDefaultTimeout(20000);
  await page.waitForLoadState('domcontentloaded');
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  const invokeOutcome = (command, args) => page.evaluate(async ({ command, args }) => {
    try { await window.__TAURI_INTERNALS__.invoke(command, args); return { ok: true, error: null }; }
    catch (error) { return { ok: false, error: { code: error?.code ?? null, message: error?.message ?? null, text: String(error) } }; }
  }, { command, args });

  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'Workload control fixture', host: 'atlas-linux' } });
  const serverId = (await invoke('map_state')).servers[0].id;
  await page.reload();
  await page.getByRole('button', { name: 'Open Workload control fixture workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Workload control fixture', exact: true });
  await workspace.getByRole('button', { name: 'Refresh inventory', exact: true }).click();
  await expect(workspace.getByRole('status')).toContainText('Approved inventory updated');
  const panel = workspace.locator('.workload-inspection');
  await panel.getByRole('button', { name: 'Open protected view', exact: true }).click();
  await expect(panel.locator('tbody tr')).toHaveCount(13);
  const dockerRow = panel.locator('tbody tr').filter({ hasText: 'atlas-fixture-db' });
  await dockerRow.getByRole('button', { name: 'Inspect', exact: true }).click();
  const dockerInspection = panel.getByRole('region', { name: 'Protected workload result' });
  await expect(dockerInspection).toContainText('Stopped');
  await expect(dockerInspection).toContainText('ATLAS_TEST_DOCKER_CONTAINER_LOG');
  results.push('inventory refresh, protected inspection/logs, and reviewed system-Docker control all used the pinned local socket despite hostile SSH DOCKER_HOST and DOCKER_CONTEXT values');

  const runAction = async (workloadName, action, expectedState, usePassword, expectedMessage) => {
    const row = panel.locator('tbody tr').filter({ hasText: workloadName });
    await row.getByRole('button', { name: 'Control…', exact: true }).click();
    const review = panel.getByRole('region', { name: 'Reviewed workload control' });
    await review.getByLabel('Workload action').selectOption(action);
    await review.getByRole('button', { name: 'Review action', exact: true }).click();
    const form = review.getByRole('form', { name: 'Confirm workload action' });
    await expect(form).toContainText(action[0].toUpperCase() + action.slice(1));
    if (usePassword) await form.getByLabel('Password for this action').fill(info.password);
    await form.getByRole('button', { name: usePassword ? 'Confirm with fresh sudo' : 'Confirm action', exact: true }).click();
    const result = review.locator('.workload-action-result');
    await expect(result).toBeVisible();
    await expect(result).toContainText('Observed complete');
    await expect(result).toContainText(expectedState);
    if (expectedMessage) await expect(result).toContainText(expectedMessage);
    results.push(`${workloadName}: reviewed ${action} returned an observed ${expectedState.toLowerCase()} state${usePassword ? ' after a fresh sudo password' : ' as the authenticated SSH account'}`);
    await review.getByRole('button', { name: 'Close', exact: true }).click();
    return result;
  };

  // A bad password consumes its review and is rejected before dispatch.
  const failedRow = panel.locator('tbody tr').filter({ hasText: 'fixture-stopped.service' });
  await failedRow.getByRole('button', { name: 'Control…', exact: true }).click();
  let review = panel.getByRole('region', { name: 'Reviewed workload control' });
  await review.getByLabel('Workload action').selectOption('start');
  await review.getByRole('button', { name: 'Review action', exact: true }).click();
  let form = review.getByRole('form', { name: 'Confirm workload action' });
  const beforeBadPassword = await stats();
  await form.getByLabel('Password for this action').fill('intentionally-wrong');
  await form.getByRole('button', { name: 'Confirm with fresh sudo', exact: true }).click();
  await expect(review.getByRole('alert')).toContainText('fresh sudo check failed');
  expect(await stats()).toEqual(beforeBadPassword);
  results.push('wrong fresh sudo password is refused before the workload command is dispatched');
  await review.getByRole('button', { name: 'Close', exact: true }).click();

  // Exercise every action for each supported adapter on controlled fixture targets.
  await runAction('fixture-stopped.service', 'start', 'Running', true);
  await runAction('fixture-stopped.service', 'stop', 'Stopped', true);
  await runAction('fixture-stopped.service', 'restart', 'Running', true);
  await runAction('fixture-worker.service', 'start', 'Running', false);
  await runAction('fixture-worker.service', 'stop', 'Stopped', false);
  await runAction('fixture-worker.service', 'restart', 'Running', false);
  await runAction('atlas-fixture-api', 'stop', 'Stopped', false);
  await runAction('atlas-fixture-api', 'start', 'Running', false);
  await runAction('atlas-fixture-api', 'restart', 'Running', false);
  await runAction('atlas-fixture-db', 'start', 'Running', true);
  await runAction('atlas-fixture-db', 'stop', 'Stopped', true);
  await runAction('atlas-fixture-db', 'restart', 'Running', true);
  expect((await stats()).control_count).toBe(12);
  results.push('system and user systemd, rootless Podman, and system Docker each completed start, stop and restart against isolated targets');

  await runAction('fixture-oneshot.service', 'start', 'Stopped', true, 'one-shot service is inactive after its task finished');
  await runAction('fixture-oneshot.service', 'restart', 'Stopped', true, 'one-shot service is inactive after its task finished');
  expect((await stats()).control_count).toBe(14);
  results.push('successful systemd one-shot start and restart are reported completed while their observed active state correctly remains stopped');

  const slowRow = panel.locator('tbody tr').filter({ hasText: 'fixture-slow.service' });
  await slowRow.getByRole('button', { name: 'Control…', exact: true }).click();
  review = panel.getByRole('region', { name: 'Reviewed workload control' });
  await review.getByLabel('Workload action').selectOption('start');
  await review.getByRole('button', { name: 'Review action', exact: true }).click();
  form = review.getByRole('form', { name: 'Confirm workload action' });
  const beforeActiveCancel = await stats();
  await form.getByRole('button', { name: 'Confirm action', exact: true }).click();
  await form.getByRole('button', { name: 'Cancel action', exact: true }).click();
  await expect(panel.locator('.workload-control-uncertain')).toContainText('result is unknown', { timeout: 30000 });
  await delay(7500);
  const afterActiveCancel = await stats();
  expect(afterActiveCancel.control_count - beforeActiveCancel.control_count).toBeLessThanOrEqual(1);
  results.push('cancelling an in-flight native action never reports success; a delayed fixture request is not dispatched twice');
  await panel.locator('.workload-control-uncertain').getByRole('button', { name: 'Dismiss', exact: true }).click();
  await expect(panel.locator('.workload-control-uncertain')).toHaveCount(0);
  await review.getByRole('button', { name: 'Close', exact: true }).click();

  const beforeLockInFlight = await stats();
  await slowRow.getByRole('button', { name: 'Control…', exact: true }).click();
  review = panel.getByRole('region', { name: 'Reviewed workload control' });
  await review.getByLabel('Workload action').selectOption('start');
  await review.getByRole('button', { name: 'Review action', exact: true }).click();
  form = review.getByRole('form', { name: 'Confirm workload action' });
  await form.getByRole('button', { name: 'Confirm action', exact: true }).click();
  await panel.getByRole('button', { name: 'Lock protected view', exact: true }).click();
  await expect(panel.getByRole('button', { name: 'Open protected view', exact: true })).toBeVisible();
  await expect(panel.locator('.workload-control-uncertain')).toContainText('result is unknown');
  await expect(panel.getByRole('region', { name: 'Reviewed workload control' })).toHaveCount(0);
  await delay(7500);
  const afterLockInFlight = await stats();
  expect(afterLockInFlight.control_count - beforeLockInFlight.control_count).toBeLessThanOrEqual(1);
  results.push('locking while a control request is in flight clears protected state, preserves an uncertainty notice, and does not duplicate dispatch');
  await panel.getByRole('button', { name: 'Open protected view', exact: true }).click();
  await expect(panel.locator('tbody tr')).toHaveCount(13);

  const disconnectedRow = panel.locator('tbody tr').filter({ hasText: 'fixture-disconnect.service' });
  await disconnectedRow.getByRole('button', { name: 'Control…', exact: true }).click();
  review = panel.getByRole('region', { name: 'Reviewed workload control' });
  await review.getByLabel('Workload action').selectOption('start');
  await review.getByRole('button', { name: 'Review action', exact: true }).click();
  form = review.getByRole('form', { name: 'Confirm workload action' });
  const beforeDisconnect = await stats();
  await form.getByLabel('Password for this action').fill(info.password);
  await form.getByRole('button', { name: 'Confirm with fresh sudo', exact: true }).click();
  await expect(panel.locator('.workload-control-uncertain')).toContainText('result is unknown');
  const afterDisconnect = await stats();
  expect(afterDisconnect.control_count).toBe(beforeDisconnect.control_count + 1);
  expect(afterDisconnect.disconnect_target_found).toBe(true);
  await expect(panel.getByRole('button', { name: 'Open protected view', exact: true })).toBeVisible();
  results.push('a fixture SSH channel terminated after the service action was dispatched; the uncertain-result warning remains visible after the protected session is cleared');
  await panel.getByRole('button', { name: 'Open protected view', exact: true }).click();
  await expect(panel.locator('tbody tr')).toHaveCount(13);
  const disconnectedInspection = panel.locator('tbody tr').filter({ hasText: 'fixture-disconnect.service' });
  await disconnectedInspection.getByRole('button', { name: 'Inspect', exact: true }).click();
  await expect(panel.getByRole('region', { name: 'Protected workload result' })).toContainText('Running');
  await expect(panel.locator('.workload-control-uncertain')).toBeVisible();
  results.push('reconnecting and inspecting confirms the service reached running after the response channel was lost');

  // Backend scope, expiry, cancellation and idempotency checks use a second fixture session.
  const other = await invoke('add_server', { input: { name: 'Wrong control target', host: 'other-fixture.home' } });
  const otherId = other.servers.find(server => server.name === 'Wrong control target').id;
  const ticket = await invoke('files_prepare', { serverId });
  const session = await invoke('files_open', { serverId, operationId: ticket.operation_id });
  const systemService = session.workloads.find(item => item.name === 'atlas-fixture.service');
  const serviceId = systemService.id;
  const deniedOutcome = outcome => {
    const details = JSON.stringify(outcome.error ?? {}).toLowerCase();
    return !outcome.ok && (details.includes('denied') || details.includes('not permitted'));
  };
  const unknownResult = await invokeOutcome('workload_control_preview', { serverId, sessionId: session.session_id, workloadId: `wl1_${'a'.repeat(64)}`, action: 'start' });
  const wrongHostResult = await invokeOutcome('workload_control_preview', { serverId: otherId, sessionId: session.session_id, workloadId: serviceId, action: 'start' });
  const unknownDenied = deniedOutcome(unknownResult), wrongHostDenied = deniedOutcome(wrongHostResult);
  if (!unknownDenied || !wrongHostDenied) console.log(JSON.stringify({ unknownResult, wrongHostResult }));
  expect({ unknownDenied, wrongHostDenied }).toEqual({ unknownDenied: true, wrongHostDenied: true });
  results.push('unknown workload IDs and a session used against another saved host are denied before dispatch');

  const beforeCancelled = await stats();
  const cancelledPreview = await invoke('workload_control_preview', { serverId, sessionId: session.session_id, workloadId: serviceId, action: 'restart' });
  await invoke('elevation_cancel', { serverId, sessionId: session.session_id, operationId: cancelledPreview.operation_id });
  const cancelledConfirm = await invokeOutcome('workload_control_confirm', { serverId, sessionId: session.session_id, operationId: cancelledPreview.operation_id, password: info.password });
  expect(cancelledConfirm.ok).toBe(false);
  expect(await stats()).toEqual(beforeCancelled);
  results.push('a cancelled action ticket cannot be confirmed or dispatched afterward');

  const userService = session.workloads.find(item => item.name === 'fixture-worker.service');
  const duplicatePreview = await invoke('workload_control_preview', { serverId, sessionId: session.session_id, workloadId: userService.id, action: 'start' });
  const beforeDuplicate = await stats();
  await invoke('workload_control_confirm', { serverId, sessionId: session.session_id, operationId: duplicatePreview.operation_id });
  const afterSingle = await stats();
  expect(afterSingle.control_count).toBe(beforeDuplicate.control_count + 1);
  let duplicateRejected = false;
  try { await invoke('workload_control_confirm', { serverId, sessionId: session.session_id, operationId: duplicatePreview.operation_id }); }
  catch (error) { duplicateRejected = true; }
  expect(duplicateRejected).toBe(true);
  expect(await stats()).toEqual(afterSingle);
  results.push('repeating a confirmed operation ID is rejected and produces no second control command');

  const expiryPreview = await invoke('workload_control_preview', { serverId, sessionId: session.session_id, workloadId: serviceId, action: 'stop' });
  const beforeExpiry = await stats();
  await invoke('files_lock', { sessionId: session.session_id });
  const expiredConfirm = await invokeOutcome('workload_control_confirm', { serverId, sessionId: session.session_id, operationId: expiryPreview.operation_id, password: info.password });
  expect(expiredConfirm.ok).toBe(false);
  expect(await stats()).toEqual(beforeExpiry);
  results.push('locking the protected session invalidates a pending control review before dispatch');

  const beforePrivacy = await databaseSnapshot(database);
  const beforeMap = await invoke('map_state');
  await panel.getByRole('button', { name: 'Lock protected view', exact: true }).click();
  await expect(panel.getByRole('region', { name: 'Protected workload result' })).toHaveCount(0);
  await expect(panel.getByRole('region', { name: 'Reviewed workload control' })).toHaveCount(0);
  await expect(panel.locator('.workload-control-uncertain')).toBeVisible();
  expect(await databaseSnapshot(database)).toEqual(beforePrivacy);
  const afterMap = await invoke('map_state');
  expect(afterMap.servers).toEqual(beforeMap.servers);
  expect(afterMap.events).toEqual(beforeMap.events);
  const dbBytes = await readFile(database);
  for (const marker of ['ATLAS_TEST_SYSTEM_SERVICE_LOG', 'ATLAS_TEST_PODMAN_CONTAINER_LOG', 'ATLAS_TEST_DOCKER_CONTAINER_LOG']) {
    expect(dbBytes.includes(Buffer.from(marker))).toBe(false);
  }
  results.push('Lock clears protected inspection/control results; the database, inventory and activity record no service logs or action credentials');

  // Replace only the disposable fixture SSH account with one that has no sudo grant.
  const originalConfig = await readFile(info.config, 'utf8');
  const normalizedConfig = originalConfig.replaceAll('\r\n', '\n');
  const originalAlias = `Host atlas-linux\n  Hostname 127.0.0.1\n  Port ${info.port}\n  User fixture\n`;
  const changedAlias = `Host atlas-linux\n  Hostname 127.0.0.1\n  Port ${info.port}\n  User fixture-nosudo\n`;
  if (!normalizedConfig.includes(originalAlias)) throw new Error('Fixture SSH profile block was not found');
  try {
    await writeFile(info.config, normalizedConfig.replace(originalAlias, changedAlias), 'utf8');
    await panel.getByRole('button', { name: 'Open protected view', exact: true }).click();
    await expect(panel).toContainText('different SSH connection');
    await workspace.getByRole('button', { name: 'Refresh inventory', exact: true }).click();
    await expect(workspace.locator('.inventory-refresh-status').filter({ hasText: 'Approved inventory updated' })).toBeVisible();
    await panel.getByRole('button', { name: 'Lock protected view', exact: true }).click();
    await panel.getByRole('button', { name: 'Open protected view', exact: true }).click();
    await expect(panel.locator('tbody tr')).toHaveCount(13);
    const noSudoBefore = await stats();
    const noSudoRow = panel.locator('tbody tr').filter({ hasText: 'fixture-stopped.service' });
    await noSudoRow.getByRole('button', { name: 'Control…', exact: true }).click();
    review = panel.getByRole('region', { name: 'Reviewed workload control' });
    await review.getByLabel('Workload action').selectOption('start');
    await review.getByRole('button', { name: 'Review action', exact: true }).click();
    form = review.getByRole('form', { name: 'Confirm workload action' });
    await form.getByLabel('Password for this action').fill(info.password);
    await form.getByRole('button', { name: 'Confirm with fresh sudo', exact: true }).click();
    await expect(review.getByRole('alert')).toContainText('did not authorize this control action');
    expect(await stats()).toEqual(noSudoBefore);
    results.push('a correctly authenticated SSH account with no sudo policy is refused before system workload control');
  } finally {
    await writeFile(info.config, originalConfig, 'utf8');
  }

  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  expect(pageErrors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'disposable loopback SSH fixture only', realHostsUsed: false, fixtureControlCount: (await stats()).control_count, protectedOutputPersisted: false, pageErrors, directory };
  const proofPath = path.join(directory, 'result.json');
  await writeFile(proofPath, JSON.stringify(proof, null, 2));
  const proofBytes = await readFile(proofPath);
  for (const marker of [info.password, 'ATLAS_TEST_SYSTEM_SERVICE_LOG', 'ATLAS_TEST_DOCKER_CONTAINER_LOG']) {
    expect(proofBytes.includes(Buffer.from(marker))).toBe(false);
  }
  console.log(JSON.stringify(proof, null, 2));
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native workload-control proof failed in ${directory}; credentials and protected output were not written to the evidence file.`);
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
