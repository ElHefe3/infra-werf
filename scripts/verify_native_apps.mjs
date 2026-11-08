// Actual WebView2/Rust proof for remote application tunnels (F17/T035).
// Loopback SSH fixture, private loopback app and a disposable store only.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import net from 'node:net';
import os from 'node:os';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd(), directory = path.join(root, '.tools', `apps-proof-${randomUUID()}`);
await mkdir(directory, { recursive: true });
const fixture = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/apps_fixture.py', path.join(directory, 'fixture')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
const lines = createInterface({ input: fixture.stdout }); let diagnostic = ''; fixture.stderr.on('data', d => diagnostic += d.toString());
const info = await new Promise((resolve, reject) => { const t = setTimeout(() => reject(new Error(`Fixture timeout: ${diagnostic}`)), 20000); lines.once('line', l => { clearTimeout(t); resolve(JSON.parse(l)); }); });
const stats = async () => JSON.parse(await readFile(info.stats, 'utf8'));
const delay = ms => new Promise(r => setTimeout(r, ms));
const endpoint = 'http://127.0.0.1:9237', database = path.join(directory, 'native.sqlite3');
const results = [], errors = []; let app, browser, page;
const listening = (port, host = '127.0.0.1') => new Promise(resolve => { const s = net.connect({ port, host, timeout: 1500 }); s.once('connect', () => { s.destroy(); resolve(true); }); s.once('error', () => resolve(false)); s.once('timeout', () => { s.destroy(); resolve(false); }); });
const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
const rejected = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args).then(() => null, e => e?.code ?? String(e)), { command, args });
async function launch() {
  const { ATLAS_NATIVE_DB, ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  app = spawn(process.env.ATLAS_DESKTOP_BINARY ?? path.join(root, 'target/release/atlas-native.exe'), [], { cwd: root, windowsHide: true, env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_SSH_CONFIG: info.config, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9237 --remote-debugging-address=127.0.0.1' }, stdio: 'ignore' });
  for (let n = 0; n < 100; n++) { if (app.exitCode !== null) throw new Error('Native application exited'); try { if ((await fetch(endpoint + '/json/version')).ok) break; } catch {} await delay(200); }
  browser = await chromium.connectOverCDP(endpoint); page = browser.contexts()[0].pages()[0]; page.setDefaultTimeout(15000);
  page.on('pageerror', e => errors.push(e.message)); await page.waitForLoadState('domcontentloaded'); await page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__?.invoke));
}
/** The app window is a separate (InPrivate) webview; find it on a fresh CDP connection. */
async function appPage(url) {
  const origin = new URL(url).origin;
  for (let n = 0; n < 50; n++) {
    const viewer = await chromium.connectOverCDP(endpoint);
    const found = viewer.contexts().flatMap(c => c.pages()).find(p => p.url().startsWith(origin));
    if (found) return { viewer, view: found };
    await viewer.close(); await delay(200);
  }
  throw new Error(`Application window for ${origin} not found`);
}
/** Post WM_CLOSE (like the title-bar X) to every visible top-level window of
 *  the app process that shows a remote application. Returns the titles. */
async function closeAppWindows() {
  const source = `using System;using System.Text;using System.Runtime.InteropServices;public static class AW{public delegate bool P(IntPtr h,IntPtr l);[DllImport("user32.dll")]public static extern bool EnumWindows(P p,IntPtr l);[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);[DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetWindowTextW(IntPtr h,StringBuilder s,int n);[DllImport("user32.dll")]public static extern bool PostMessageW(IntPtr h,uint m,IntPtr w,IntPtr l);public static string Close(uint pid){var r=new StringBuilder();EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p);if(p!=pid||!IsWindowVisible(h))return true;var s=new StringBuilder(512);GetWindowTextW(h,s,512);var t=s.ToString();if(!t.Contains("localhost:"))return true;r.Append(t).Append("\\n");PostMessageW(h,0x10,IntPtr.Zero,IntPtr.Zero);return true;},IntPtr.Zero);return r.ToString();}}`;
  const script = `[Console]::OutputEncoding=[Text.Encoding]::UTF8; Add-Type -TypeDefinition $env:ATLAS_AW; [AW]::Close(${app.pid})`;
  const child = spawn('powershell', ['-NoProfile', '-NonInteractive', '-Command', script], { env: { ...process.env, ATLAS_AW: source }, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = ''; child.stdout.on('data', d => out += d.toString()); let err = ''; child.stderr.on('data', d => err += d.toString());
  const code = await new Promise(r => child.once('exit', r));
  const titles = out.split('\n').map(s => s.trim()).filter(Boolean);
  if (code !== 0 || !titles.length) throw new Error(`No application window closed (exit ${code}): ${err}`);
  return titles;
}
const policy = { lifecycle: 'active', enabled: false, owner_offline: false, inventory_enabled: false, interval_seconds: 60, inventory_interval_seconds: 900 };
try {
  await launch(); await invoke('monitoring_discover');
  let state = await invoke('map_state');
  const node = state.servers.find(s => s.host === 'apps-node'), forwarding = state.servers.find(s => s.host === 'apps-forwarding');
  expect(node).toBeTruthy(); expect(forwarding).toBeTruthy();
  await invoke('monitoring_configure', { serverId: node.id, policy }); await invoke('monitoring_configure', { serverId: forwarding.id, policy });

  // 1. Native UI: workspace → Apps → save a shortcut → open it.
  await page.getByRole('button', { name: `Open ${node.name} workspace`, exact: true }).click();
  await page.getByRole('tab', { name: 'Apps', exact: true }).click();
  await page.getByLabel('Remote port').fill(String(info.app_port));
  await page.getByLabel('Application name').fill('Fixture dashboard');
  await page.getByRole('button', { name: 'Save shortcut', exact: true }).click();
  await page.locator('button.app-launch', { hasText: 'Fixture dashboard' }).click();
  await expect(page.locator('.tunnel-list li')).toHaveCount(1, { timeout: 30000 });
  let [tunnel] = await invoke('tunnel_list');
  expect(tunnel.url).toBe(`http://127.0.0.1:${tunnel.local_port}/`); expect(tunnel.remote_port).toBe(info.app_port);
  const { viewer, view } = await appPage(tunnel.url);
  await expect(view.locator('#api')).toHaveText('online'); await expect(view.locator('#ws')).toHaveText('pong');
  expect((await stats()).forwards.every(([host, port]) => host === 'localhost' && port === info.app_port)).toBe(true);
  results.push('native Apps tab verifies SSH, opens an isolated window and the private app works over HTTP and WebSocket through localhost forwarding');
  console.log('PASS: open application through tunnel');

  // 2. Isolation: no InfraWerf commands, no off-origin navigation, loopback-only listener.
  const ipc = await view.evaluate(async () => { const internals = window.__TAURI_INTERNALS__; if (!internals?.invoke) return 'absent'; try { await internals.invoke('map_state'); return 'allowed'; } catch { return 'denied'; } });
  expect(ipc).not.toBe('allowed');
  await view.evaluate(() => { location.href = 'http://127.0.0.2:9/elsewhere'; }).catch(() => {}); await delay(1500);
  expect(view.url().startsWith(`http://127.0.0.1:${tunnel.local_port}/`)).toBe(true);
  const lanAddresses = Object.values(os.networkInterfaces()).flat().filter(a => a && a.family === 'IPv4' && !a.internal).map(a => a.address);
  const lan = lanAddresses.find(a => !a.startsWith('169.254.')) ?? lanAddresses[0];
  if (lan) expect(await listening(tunnel.local_port, lan)).toBe(false);
  await view.screenshot({ path: path.join(directory, 'app-window.png') });
  await page.screenshot({ path: path.join(directory, 'apps-tab.png') });
  results.push(`application window cannot invoke InfraWerf (${ipc}), stays on the tunnel origin, and the forward is not reachable on ${lan ?? 'the LAN (no interface)'}`);
  console.log('PASS: isolation');

  // 3. Closing the window (a real WM_CLOSE, like the title-bar X) ends the
  // tunnel and its SSH connection.
  await viewer.close();
  const closedTitles = await closeAppWindows();
  console.log(`Closed application window(s): ${closedTitles.join(' | ')}`);
  await expect.poll(async () => (await invoke('tunnel_list')).length, { timeout: 10000 }).toBe(0);
  expect(await listening(tunnel.local_port)).toBe(false);
  await expect.poll(async () => (await stats()).active, { timeout: 10000 }).toBe(0);
  await expect(page.locator('.tunnel-list li')).toHaveCount(0);
  results.push('closing the application window removes the listener and the SSH connection');
  console.log('PASS: window close teardown');

  // 4. Archiving the server revokes its tunnel and closes the window.
  tunnel = await invoke('tunnel_open', { serverId: node.id, request: { remote_port: info.app_port, path: '/' } });
  expect(await listening(tunnel.local_port)).toBe(true);
  await invoke('monitoring_configure', { serverId: node.id, policy: { ...policy, lifecycle: 'archived' } });
  await expect.poll(async () => (await invoke('tunnel_list')).length, { timeout: 10000 }).toBe(0);
  expect(await listening(tunnel.local_port)).toBe(false);
  expect(await rejected('tunnel_open', { serverId: node.id, request: { remote_port: info.app_port, path: '/' } })).toBe('denied');
  results.push('archiving the server ends its tunnel and refuses new ones');
  console.log('PASS: lifecycle revocation');

  // 5. Configured forwards and invalid requests are refused.
  expect(await rejected('tunnel_open', { serverId: forwarding.id, request: { remote_port: info.app_port, path: '/' } })).toBe('denied');
  expect(await listening(47999)).toBe(false);
  for (const request of [{ remote_port: 0, path: '/' }, { remote_port: 80, path: '//evil.example/' }, { remote_port: 80, path: 'relative' }])
    expect(await rejected('tunnel_open', { serverId: node.id, request })).toBe('invalid_input');
  expect((await stats()).refused_forwards).toBe(0);
  results.push('aliases with configured forwards and invalid ports/paths are refused; no unreviewed forward was requested');
  console.log('PASS: refusals');

  // 6. Quitting InfraWerf ends every tunnel.
  await invoke('monitoring_configure', { serverId: node.id, policy });
  tunnel = await invoke('tunnel_open', { serverId: node.id, request: { remote_port: info.app_port, path: '/' } });
  const second = await invoke('tunnel_open', { serverId: node.id, request: { remote_port: info.app_port, path: '/api/ping' } });
  expect(second.url.endsWith('/api/ping')).toBe(true); expect((await invoke('tunnel_list')).length).toBe(2);
  await page.getByRole('button', { name: 'Quit InfraWerf', exact: true }).click().catch(async () => { await page.keyboard.press('Escape'); await page.getByRole('button', { name: 'Quit InfraWerf', exact: true }).click(); });
  await expect.poll(() => app.exitCode, { timeout: 20000 }).not.toBe(null);
  await browser.close().catch(() => {}); browser = null;
  expect(await listening(tunnel.local_port)).toBe(false); expect(await listening(second.local_port)).toBe(false);
  await expect.poll(async () => (await stats()).active, { timeout: 10000 }).toBe(0);
  results.push('quitting InfraWerf ends all tunnels and SSH connections');
  console.log('PASS: exit teardown');

  expect(errors).toEqual([]);
  await writeFile(path.join(directory, 'result.json'), JSON.stringify({ result: 'pass', results, errors, directory, http: (await stats()).http }, null, 2));
  console.log(JSON.stringify({ result: 'pass', results, directory }, null, 2));
} catch (error) {
  console.error(`Apps proof artifacts: ${directory}`); await page?.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {}); throw error;
} finally {
  await browser?.close().catch(() => {}); if (app?.exitCode === null) { app.kill(); await new Promise(r => app.once('exit', r)); }
  if (fixture.exitCode === null) { fixture.stdin.end('quit\n'); await Promise.race([new Promise(r => fixture.once('exit', r)), delay(5000)]); if (fixture.exitCode === null) fixture.kill(); }
  lines.close();
}
