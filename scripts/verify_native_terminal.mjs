// Actual Windows Tauri/WebView2 terminal journey against the disposable SSH fixture only.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir, rm } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import net from 'node:net';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-terminal-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const digest = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const reservePort = async () => {
  const listener = net.createServer();
  await new Promise((resolve, reject) => listener.once('error', reject).listen(0, '127.0.0.1', resolve));
  const port = listener.address().port;
  await new Promise(resolve => listener.close(resolve));
  return port;
};
const fixtureDirectory = path.join(directory, 'fixture');
const server = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/linux_fixture.py', fixtureDirectory], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
const lines = createInterface({ input: server.stdout });
let diagnostic = '';
server.stderr.on('data', data => { diagnostic += data.toString().slice(0, 1000); });
let app, browser, page, info;
const results = [];
const errors = [];
try {
  info = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`SSH fixture startup failed: ${diagnostic}`)), 30000);
    lines.once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); });
    server.once('exit', code => { if (code) { clearTimeout(timer); reject(new Error(`SSH fixture exited: ${diagnostic}`)); } });
  });
  const configHash = await digest(info.config), trustHash = await digest(info.known_hosts);
  const cliLog = path.join(directory, 'cli-terminal.log');
  const cli = spawn('C:/Windows/System32/OpenSSH/ssh.exe', [
    '-F', info.config,
    '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
    '-o', 'NoHostAuthenticationForLocalhost=no', '-o', 'FingerprintHash=sha256',
    '-o', 'LogLevel=QUIET',
    '-o', 'LogVerbose=*sshconnect.c:verify_host_key():*,*sshconnect.c:check_host_key():*,*sshconnect2.c:ssh_userauth2():*',
    '-o', 'PasswordAuthentication=no', '-o', 'KbdInteractiveAuthentication=no',
    '-o', 'PreferredAuthentications=publickey', '-o', 'NumberOfPasswordPrompts=0',
    '-o', 'ControlMaster=no', '-o', 'ControlPath=none', '-o', 'ControlPersist=no',
    '-o', 'ForkAfterAuthentication=no', '-o', 'PermitLocalCommand=no',
    '-o', 'ClearAllForwardings=yes', '-o', 'ForwardAgent=no', '-o', 'ForwardX11=no',
    '-o', 'RequestTTY=no', '-o', 'RemoteCommand=none', '-o', 'ConnectTimeout=5',
    '-o', 'ConnectionAttempts=1', '-o', 'ServerAliveInterval=2', '-o', 'ServerAliveCountMax=2',
    '-E', cliLog, '-tt', '-o', 'SessionType=default',
    '-o', 'KnownHostsCommand=none', '-o', 'ProxyCommand=none', '-o', 'ProxyJump=none',
    '-o', 'Hostname=127.0.0.1', '-l', 'fixture', '-p', String(info.port), '--', 'atlas-linux',
  ], { windowsHide: true, stdio: ['pipe', 'ignore', 'ignore'] });
  cli.stdin.end('exit\r');
  const cliExit = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => { cli.kill(); reject(new Error('Direct OpenSSH terminal login probe timed out')); }, 10000);
    cli.once('error', error => { clearTimeout(timer); reject(error); });
    cli.once('exit', (code, signal) => { clearTimeout(timer); resolve({ code, signal }); });
  });
  const cliDiagnostics = await readFile(cliLog, 'utf8').catch(() => '');
  await rm(cliLog, { force: true });
  const cliAuth = cliDiagnostics.includes('Authenticated to') && cliDiagnostics.includes('using "publickey".');
  if (cliExit.code !== 0 || !cliAuth) {
    throw new Error(`Direct OpenSSH terminal login probe failed: ${JSON.stringify({ ...cliExit, logBytes: cliDiagnostics.length, hostKeyEvidence: cliDiagnostics.includes('Server host key:'), authenticatedPublicKey: cliAuth })}`);
  }
  results.push('System OpenSSH accepted the fixture host key and authenticated the terminal login');
  const database = path.join(directory, 'native.sqlite3');
  const terminalTemp = path.join(directory, 'terminal-tmp');
  await mkdir(terminalTemp);
  const debugPort = await reservePort();
  const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  app = spawn(path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root,
    windowsHide: true,
    env: {
      ...environment,
      ATLAS_NATIVE_DB: database,
      ATLAS_SSH_CONFIG: info.config,
      WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${debugPort} --remote-debugging-address=127.0.0.1`,
      TEMP: terminalTemp,
      TMP: terminalTemp,
    },
    stdio: 'ignore',
  });
  for (let i = 0; i < 120; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch(`http://127.0.0.1:${debugPort}/json/version`)).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${debugPort}`);
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  page.on('pageerror', error => errors.push(error.message));
  const invoke = (command, args) => page.evaluate(async ({ command, args }) => {
    try { return await window.__TAURI_INTERNALS__.invoke(command, args); }
    catch (error) {
      const detail = typeof error === 'string' ? error : JSON.stringify(error);
      throw new Error(`${command} failed: ${detail ?? String(error)}`);
    }
  }, { command, args });
  const rejected = (command, args) => page.evaluate(async ({ command, args }) => {
    try { await window.__TAURI_INTERNALS__.invoke(command, args); return false; }
    catch { return true; }
  }, { command, args });
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'Terminal fixture', host: 'atlas-linux' } });
  const mapBefore = await invoke('map_state');
  const serverId = mapBefore.servers[0].id;
  const dbBefore = await digest(database);

  // Exercise the protected backend stream first, including an explicit PTY resize.
  const ticket = await invoke('files_prepare', { serverId });
  const protectedSession = await invoke('files_open', { serverId, operationId: ticket.operation_id });
  const sessionId = protectedSession.session_id;
  await invoke('files_terminal_start', { serverId, sessionId, cols: 80, rows: 24 });
  const directMarker = `ATLAS_DIRECT_${randomUUID().replaceAll('-', '')}`;
  const write = async value => invoke('files_terminal_write', { serverId, sessionId, data: Array.from(new TextEncoder().encode(value)) });
  let directBytes = Buffer.alloc(0);
  const readUntil = async (pattern, timeout = 12000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const result = await invoke('files_terminal_poll', { serverId, sessionId });
      if (result.overflow) throw new Error('Terminal output buffer overflowed during the fixture journey');
      if (result.data.length) directBytes = Buffer.concat([directBytes, Buffer.from(result.data)]);
      const text = directBytes.toString('utf8');
      if (pattern.test(text)) return text;
      if (result.ended) throw new Error('Fixture shell ended before expected output arrived');
      await delay(25);
    }
    throw new Error(`Terminal output did not match ${pattern}`);
  };
  await write(`printf '\\n${directMarker} café 世界\\n'\r`);
  const directText = await readUntil(new RegExp(`${directMarker} café 世界`));
  expect(directText).toContain(`${directMarker} café 世界`);
  await invoke('files_terminal_resize', { serverId, sessionId, cols: 73, rows: 33 });
  await write("stty size | sed 's/^/ATLAS_DIRECT_SIZE_/'\r");
  const resizedText = await readUntil(/ATLAS_DIRECT_SIZE_33\s+73/);
  expect(resizedText).toMatch(/ATLAS_DIRECT_SIZE_33\s+73/);
  await invoke('files_lock', { sessionId });
  expect(await rejected('files_terminal_write', { serverId, sessionId, data: [120, 10] })).toBe(true);
  expect(await invoke('workspace_status')).toMatchObject({ pending_operations: 0 });
  results.push('Windows protected SSH session accepted UTF-8 input/output, resized the remote PTY to 73×33, and rejected writes after Lock');

  await page.reload();
  await page.waitForLoadState('domcontentloaded');
  await page.getByRole('button', { name: 'Open Terminal fixture workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Terminal fixture', exact: true });
  await workspace.getByRole('tab', { name: 'Terminal', exact: true }).click();
  const screen = workspace.locator('.terminal-screen');
  const rows = screen.locator('.xterm-rows');
  await workspace.getByRole('button', { name: 'Connect terminal', exact: true }).click();
  await expect(workspace.locator('.terminal-state')).toHaveText('Connected');
  const textarea = screen.locator('.xterm-helper-textarea');
  const sendUi = async text => {
    await textarea.click();
    await page.keyboard.insertText(text);
    await page.keyboard.press('Enter');
  };
  const uiMarker = `ATLAS_UI_${randomUUID().replaceAll('-', '')}`;
  await sendUi(`printf '\\n${uiMarker} café 世界\\n'`);
  await expect.poll(async () => (await rows.innerText()).split(uiMarker).length - 1).toBeGreaterThanOrEqual(2);
  await expect(rows).toContainText('café 世界');

  const currentSize = async (prefix, command) => {
    await sendUi(command);
    let result = null;
    await expect.poll(async () => {
      const text = await rows.innerText();
      const matches = [...text.matchAll(new RegExp(`${prefix}(\\d+)\\s+(\\d+)`, 'g'))];
      result = matches.length ? { rows: Number(matches.at(-1)[1]), cols: Number(matches.at(-1)[2]) } : null;
      return result;
    }).not.toBeNull();
    return result;
  };
  const beforeResize = await currentSize('ATLAS_UI_SIZE_A_', "stty size | sed 's/^/ATLAS_UI_SIZE_A_/'");
  await screen.evaluate(element => {
    element.style.flex = 'none';
    element.style.width = '520px';
    element.style.height = '300px';
  });
  const afterResize = await currentSize('ATLAS_UI_SIZE_B_', "stty size | sed 's/^/ATLAS_UI_SIZE_B_/'");
  expect(afterResize.cols).toBeLessThan(beforeResize.cols);
  results.push('xterm keyboard entry passed the actual native SSH channel and the resize observer changed the remote terminal width');

  await sendUi('sleep 20');
  await delay(250);
  await page.keyboard.press('Control+C');
  const interruptMarker = `ATLAS_INTERRUPT_${randomUUID().replaceAll('-', '')}`;
  await sendUi(`printf '\\n${interruptMarker}\\n'`);
  await expect.poll(async () => (await rows.innerText()).split(interruptMarker).length - 1).toBeGreaterThanOrEqual(2);
  results.push('Ctrl+C interrupted a foreground process and the same authenticated shell accepted a subsequent command');

  await workspace.getByRole('button', { name: 'Lock session', exact: true }).click();
  await expect(workspace.locator('.terminal-state')).toHaveText('Locked');
  await expect(rows).toBeEmpty();
  const staleMarker = `ATLAS_STALE_${randomUUID().replaceAll('-', '')}`;
  await textarea.click();
  await page.keyboard.insertText(`printf '${staleMarker}'`);
  await page.keyboard.press('Enter');
  expect(await rows.innerText()).not.toContain(staleMarker);
  await workspace.getByRole('button', { name: 'Connect terminal', exact: true }).click();
  await expect(workspace.locator('.terminal-state')).toHaveText('Connected');
  await delay(500);
  expect(await rows.innerText()).not.toContain(staleMarker);
  const reconnectedMarker = `ATLAS_RECONNECTED_${randomUUID().replaceAll('-', '')}`;
  await sendUi(`printf '\\n${reconnectedMarker}\\n'`);
  await expect.poll(async () => (await rows.innerText()).split(reconnectedMarker).length - 1).toBeGreaterThanOrEqual(2);
  results.push('Lock cleared xterm scrollback; locked keystrokes were not replayed after fresh authentication');

  await workspace.getByRole('button', { name: 'Lock session', exact: true }).click();
  await expect(workspace.locator('.terminal-state')).toHaveText('Locked');
  const mapAfter = await invoke('map_state');
  expect(mapAfter.servers).toEqual(mapBefore.servers);
  expect(mapAfter.edges).toEqual(mapBefore.edges);
  expect(mapAfter.events).toEqual(mapBefore.events);
  expect(await digest(database)).toBe(dbBefore);
  expect(await digest(info.config)).toBe(configHash);
  expect(await digest(info.known_hosts)).toBe(trustHash);
  for (const file of [database, `${database}-wal`, `${database}-shm`]) {
    try {
      const bytes = await readFile(file);
      for (const marker of [directMarker, uiMarker, interruptMarker, reconnectedMarker]) {
        expect(bytes.includes(Buffer.from(marker))).toBe(false);
      }
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  await expect.poll(() => app.exitCode).toBe(null);
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
  await writeFile(path.join(directory, 'result.json'), JSON.stringify({
    result: 'pass',
    results,
    targets: 'disposable loopback OpenSSH fixture only',
    noSudo: true,
    noRealHost: true,
    sshConfigAndTrustUnchanged: true,
    terminalContentAbsentFromDatabase: true,
    pageErrors: errors,
    directory,
  }, null, 2));
  console.log(JSON.stringify({ result: 'pass', results, targets: 'disposable loopback OpenSSH fixture only', noSudo: true, noRealHost: true, sshConfigAndTrustUnchanged: true, terminalContentAbsentFromDatabase: true, pageErrors: errors, directory }, null, 2));
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations, { timeout: 15000 }).toBe(0);
  await page.getByRole('button', { name: 'Quit InfraWerf', exact: true }).click();
  await expect.poll(() => app.exitCode, { timeout: 15000 }).not.toBe(null);
} catch (failure) {
  console.error(`Native terminal proof artifacts: ${directory}`);
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
  await rm(path.join(fixtureDirectory, 'fixture-only-identity'), { force: true }).catch(() => {});
  lines.close();
}
