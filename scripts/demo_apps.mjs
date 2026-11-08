// Visible demo of remote application tunnels on disposable data (F17/T035).
// Starts the loopback fixture, opens the release build with a throwaway
// workspace, shows the Apps tab and opens the demo dashboard. Runs until the
// InfraWerf window is closed, then stops the fixture. No owner data or hosts.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd(), directory = path.join(root, '.tools', `apps-demo-${randomUUID()}`);
await mkdir(directory, { recursive: true });
const fixture = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/apps_fixture.py', path.join(directory, 'fixture')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'inherit'] });
const lines = createInterface({ input: fixture.stdout });
const info = await new Promise(resolve => lines.once('line', l => resolve(JSON.parse(l))));
const delay = ms => new Promise(r => setTimeout(r, ms));
const { ATLAS_NATIVE_DB, ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
const app = spawn(process.env.ATLAS_DESKTOP_BINARY ?? path.join(root, 'target/release/atlas-native.exe'), [], { cwd: root, env: { ...environment, ATLAS_NATIVE_DB: path.join(directory, 'demo.sqlite3'), ATLAS_SSH_CONFIG: info.config, WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9238 --remote-debugging-address=127.0.0.1' }, stdio: 'ignore' });
for (let n = 0; n < 100; n++) { try { if ((await fetch('http://127.0.0.1:9238/json/version')).ok) break; } catch {} await delay(200); }
const browser = await chromium.connectOverCDP('http://127.0.0.1:9238');
const page = browser.contexts()[0].pages()[0];
await page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__?.invoke));
const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
await invoke('monitoring_discover');
const state = await invoke('map_state');
const policy = { lifecycle: 'active', enabled: true, owner_offline: false, inventory_enabled: false, interval_seconds: 30, inventory_interval_seconds: 900 };
const node = state.servers.find(s => s.host === 'apps-node');
await invoke('update_server', { id: node.id, input: { name: 'demo-server', host: 'apps-node' } }).catch(() => {});
await invoke('monitoring_configure', { serverId: node.id, policy });
await page.reload(); await page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__?.invoke));
const label = (await invoke('map_state')).servers.find(s => s.id === node.id).name;
await page.getByRole('button', { name: `Open ${label} workspace`, exact: true }).click();
await page.getByRole('tab', { name: 'Apps', exact: true }).click();
await page.getByLabel('Remote port').fill(String(info.app_port));
await page.getByLabel('Application name').fill('Private dashboard');
await page.getByRole('button', { name: 'Save shortcut', exact: true }).click();
await page.locator('button.app-launch', { hasText: 'Private dashboard' }).click();
await browser.close();
console.log(`Demo running (fixture app on server localhost:${info.app_port}). Close InfraWerf to finish.`);
await new Promise(r => app.once('exit', r));
fixture.stdin.end('quit\n'); await Promise.race([new Promise(r => fixture.once('exit', r)), delay(5000)]); if (fixture.exitCode === null) fixture.kill();
console.log('Demo finished.');
