// Actual Tauri/WebView2 + Rust + system OpenSSH, with a loopback-only fixture.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-ssh-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const digest = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const server = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/ssh_fixture.py', path.join(directory, 'fixture')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
let diagnostic = '';
server.stderr.on('data', data => { diagnostic += data.toString().slice(0, 1000); });
const lines = createInterface({ input: server.stdout });
let app, browser, page;
const results = [];
try {
  const info = await new Promise((resolve, reject) => { const timer = setTimeout(() => reject(new Error(`SSH fixture startup failed: ${diagnostic}`)), 30000); lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); }); server.once('exit', code => { if (code) { clearTimeout(timer); reject(new Error(`SSH fixture exited: ${diagnostic}`)); } }); });
  const beforeConfig = await digest(info.config), beforeTrust = await digest(info.known_hosts);
  const beforeFallback = await digest(info.fallback_known_hosts);
  const database = path.join(directory, 'native.sqlite3');
  const { ATLAS_LEGACY_DB, ...environment } = process.env;
  app = spawn(path.join(root, 'target/debug/atlas-native.exe'), [], { cwd: root, windowsHide: true, env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_SSH_CONFIG: info.config, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9227 --remote-debugging-address=127.0.0.1' }, stdio: 'ignore' });
  for (let i = 0; i < 60; i++) { if (app.exitCode !== null) throw new Error('Native app exited during startup'); try { if ((await fetch('http://127.0.0.1:9227/json/version')).ok) break; } catch {} await delay(200); }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9227');
  page = browser.contexts()[0].pages()[0];
  await page.waitForLoadState('domcontentloaded');
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  // Catch IPC rejections inside the page so Playwright does not turn a typed
  // plain-object AtlasError into its own unstructured evaluation exception.
  const rejection = (command, args) => page.evaluate(async ({ command, args }) => { try { await window.__TAURI_INTERNALS__.invoke(command, args); return { unexpectedSuccess: true }; } catch (failure) { return failure; } }, { command, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  const aliases = ['trusted', 'agent', 'unknown', 'changed', 'rejected', 'unreachable', 'proxy', 'overflow', 'slow', 'certified', 'no-auth'];
  for (const name of aliases) await invoke('add_server', { input: { name: `${name} fixture`, host: `atlas-${name}` } });
  await page.reload(); await expect(page.locator('.server-card')).toHaveCount(aliases.length);
  const before = await invoke('map_state'); const beforeDb = await digest(database);
  async function settings(name) {
    await page.getByRole('button', { name: `Open ${name} fixture workspace`, exact: true }).click();
    const dialog = page.getByRole('dialog', { name: `${name} fixture`, exact: true });
    await dialog.getByRole('tab', { name: 'Settings', exact: true }).click(); return dialog;
  }
  for (const [name, state, title] of [['trusted', 'verified', 'SSH access verified'], ['agent', 'verified', 'SSH access verified'], ['unknown', 'unknown_host', 'Host identity needs verification'], ['changed', 'changed_host', 'Host identity has changed'], ['rejected', 'authentication_failed', 'SSH authentication failed'], ['unreachable', 'unreachable', 'SSH server could not be reached'], ['proxy', 'unsupported_configuration', 'SSH configuration needs attention'], ['certified', 'verified', 'SSH access verified'], ['no-auth', 'authentication_failed', 'SSH authentication failed']]) {
    const dialog = await settings(name);
    await dialog.getByRole('button', { name: 'Verify SSH access', exact: true }).click();
    await expect(dialog.locator(`[data-ssh-state="${state}"]`)).toBeVisible();
    await expect(dialog.getByRole('heading', { name: title, exact: true })).toBeVisible();
    if (name === 'trusted') {
      await expect(dialog.getByText(`ssh-ed25519 ${info.fingerprint}`, { exact: true })).toBeVisible();
      await expect(dialog.getByText(/no persistent SSH session is open/)).toBeVisible();
      await page.screenshot({ path: path.join(directory, 'native-ssh-verified.png') });
    }
    if (name === 'changed') await page.screenshot({ path: path.join(directory, 'native-ssh-changed.png') });
    expect(await dialog.innerText()).not.toContain('fixture-only-identity');
    expect(await dialog.innerText()).not.toContain('synthetic-protected-output');
    await page.keyboard.press('Escape'); await expect(dialog).not.toBeVisible(); results.push(`${name}: ${state}`);
  }
  let dialog = await settings('overflow');
  await dialog.getByRole('button', { name: 'Verify SSH access', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('more output than allowed');
  await page.keyboard.press('Escape'); await expect(dialog).not.toBeVisible(); results.push('overflow: bounded error without partial payload');
  dialog = await settings('slow');
  await dialog.getByRole('button', { name: 'Verify SSH access', exact: true }).click();
  await expect(dialog.getByRole('button', { name: 'Cancel check', exact: true })).toBeEnabled();
  await expect.poll(async () => JSON.parse(await readFile(info.stats, 'utf8')).slow.commands).toBe(1);
  await invoke('plugin:window|close', { label: 'main' });
  await expect(dialog.getByRole('alert')).toContainText('finishing a local operation');
  expect(app.exitCode).toBe(null);
  await dialog.getByRole('button', { name: 'Cancel check', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('cancelled');
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(0);
  await expect.poll(async () => JSON.parse(await readFile(info.stats, 'utf8')).slow.active).toBe(0);
  await page.screenshot({ path: path.join(directory, 'native-ssh-cancelled.png') });
  await page.keyboard.press('Escape'); results.push('cancel: native close prevented during real SSH work; child/lease cleaned up');

  const trusted = before.servers.find(server => server.host === 'atlas-trusted');
  const changed = before.servers.find(server => server.host === 'atlas-changed');
  const ticket = await invoke('ssh_prepare', { serverId: trusted.id });
  const denial = await rejection('ssh_verify', { serverId: changed.id, operationId: ticket.operation_id });
  expect(denial.code).toBe('denied'); await invoke('ssh_cancel', { operationId: ticket.operation_id });
  expect((await rejection('ssh_prepare', { serverId: 'missing-server' })).code).toBe('not_found');
  const unknownCommand = await rejection('run_shell', { command: 'synthetic-command' });
  expect(unknownCommand).toMatch(/not found/i);
  results.push('forged ticket/server and generic shell command denied');
  const after = await invoke('map_state');
  expect(after.servers).toEqual(before.servers); expect(after.events).toEqual(before.events); expect(after.edges).toEqual(before.edges);
  expect(await digest(database)).toBe(beforeDb); expect(await digest(info.config)).toBe(beforeConfig); expect(await digest(info.known_hosts)).toBe(beforeTrust);
  expect(await digest(info.fallback_known_hosts)).toBe(beforeFallback);
  expect(errors).toEqual([]);
  const stats = JSON.parse(await readFile(info.stats, 'utf8'));
  const agent = JSON.parse(await readFile(path.join(info.directory,'agent-stats.json'),'utf8'));
  expect(agent.identities).toBeGreaterThan(0); expect(agent.signatures).toBe(1); expect(agent.active).toBe(0);
  expect(stats.unknown.auth_attempts).toBe(0); expect(stats.changed.auth_attempts).toBe(0);
  const proof = { result: 'pass', results, agent, configTrustDatabaseUnchanged: true, stoppedBeforeUnknownChangedAuth: true, pageErrors: errors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await page.getByRole('button', { name: 'Quit InfraWerf', exact: true }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  await page?.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
  console.error(`Native SSH proof artifacts: ${directory}`); throw failure;
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
