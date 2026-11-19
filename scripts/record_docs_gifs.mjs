// Records short GIFs of the real InfraWerf app for the documentation.
// Uses the release build, a throwaway workspace and disposable loopback fixtures only:
// no owner data and no real hosts. Needs ffmpeg on PATH. Build first.
//
//   mise run exec -- node scripts/record_docs_gifs.mjs map|files|all [--out docs/images]
import { chromium } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile, rm } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';
import net from 'node:net';
import http from 'node:http';

const root = process.cwd();
const delay = ms => new Promise(r => setTimeout(r, ms));
const outIndex = process.argv.indexOf('--out');
const outDir = path.resolve(root, outIndex > 0 ? process.argv[outIndex + 1] : 'docs/images');
const WIDTH = 1200, HEIGHT = 760;

async function launch(directory, extra = {}) {
  const port = 9240 + Math.floor(Math.random() * 50);
  const { ATLAS_NATIVE_DB, ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  const app = spawn(process.env.ATLAS_DESKTOP_BINARY ?? path.join(root, 'target/release/atlas-native.exe'), [], {
    cwd: root,
    env: {
      ...environment,
      ATLAS_NATIVE_DB: path.join(directory, 'docs.sqlite3'),
      ATLAS_SSH_CONFIG: extra.sshConfig ?? path.join(directory, 'ssh_config'),
      WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    },
    stdio: 'ignore',
  });
  for (let n = 0; n < 150; n++) { try { if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break; } catch {} await delay(200); }
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const page = browser.contexts()[0].pages()[0];
  await page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__?.invoke));
  await page.setViewportSize({ width: WIDTH, height: HEIGHT });
  await installCursor(page);
  const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  return { app, browser, page, invoke };
}

// A visible pointer, because synthetic mouse moves are not drawn in screencast frames.
async function installCursor(page) {
  const script = () => {
    const make = () => {
      if (document.getElementById('docs-cursor')) return;
      const dot = document.createElement('div'); dot.id = 'docs-cursor';
      dot.style.cssText = 'position:fixed;z-index:2147483647;width:18px;height:18px;margin:-4px 0 0 -4px;border-radius:50%;background:rgba(20,20,20,.55);border:2px solid #fff;box-shadow:0 0 0 1px rgba(0,0,0,.45);pointer-events:none;transition:transform .05s;left:-40px;top:-40px';
      document.documentElement.appendChild(dot);
      addEventListener('mousemove', e => { dot.style.left = e.clientX + 'px'; dot.style.top = e.clientY + 'px'; }, true);
      addEventListener('mousedown', () => { dot.style.transform = 'scale(.7)'; }, true);
      addEventListener('mouseup', () => { dot.style.transform = 'scale(1)'; }, true);
    };
    if (document.documentElement) make(); else addEventListener('DOMContentLoaded', make);
  };
  await page.addInitScript(script);
  await page.evaluate(script);
}

let pointer = { x: 80, y: 80 };
async function move(page, locator) {
  const box = await locator.boundingBox();
  const x = box.x + box.width / 2, y = box.y + box.height / 2;
  await page.mouse.move(pointer.x, pointer.y);
  await page.mouse.move(x, y, { steps: 18 });
  pointer = { x, y };
}
async function click(page, locator, pause = 450) { await move(page, locator); await delay(220); await locator.click(); await delay(pause); }
async function typeInto(page, locator, text) { await move(page, locator); await locator.click(); await locator.press('Control+A'); await locator.pressSequentially(text, { delay: 35 }); await delay(250); }

async function record(page, name, steps) {
  const client = await page.context().newCDPSession(page);
  const frames = [];
  client.on('Page.screencastFrame', ({ data, metadata, sessionId }) => {
    frames.push({ t: metadata.timestamp ?? Date.now() / 1000, data });
    client.send('Page.screencastFrameAck', { sessionId }).catch(() => {});
  });
  await client.send('Page.startScreencast', { format: 'jpeg', quality: 88, everyNthFrame: 1 });
  await delay(400);
  await steps();
  await delay(1200);
  await client.send('Page.stopScreencast');
  await mkdir(outDir, { recursive: true });
  const work = path.join(outDir, `.frames-${name}`);
  await rm(work, { recursive: true, force: true }); await mkdir(work, { recursive: true });
  let list = '';
  for (let i = 0; i < frames.length; i++) {
    const file = path.join(work, `f${String(i).padStart(5, '0')}.jpg`);
    await writeFile(file, Buffer.from(frames[i].data, 'base64'));
    const next = frames[i + 1]?.t ?? frames[i].t + 1.6;
    list += `file '${file.replace(/\\/g, '/')}'\nduration ${Math.min(Math.max(next - frames[i].t, 0.04), 2.2).toFixed(3)}\n`;
  }
  if (frames.length) list += `file '${path.join(work, `f${String(frames.length - 1).padStart(5, '0')}.jpg`).replace(/\\/g, '/')}'\n`;
  const listFile = path.join(work, 'list.txt'); await writeFile(listFile, list);
  const gif = path.join(outDir, `${name}.gif`);
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', listFile, '-vf',
'fps=8,scale=880:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=96:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle', '-loop', '0', gif]);
  await rm(work, { recursive: true, force: true });
  console.log(`${path.relative(root, gif)}: ${frames.length} frames`);
}

// The glass look needs a see-through window behind it; recordings use the classic look in light mode.
async function classicLight(page) {
  const classic = page.getByRole('button', { name: /Classic look/ });
  if (await classic.count()) await classic.click();
  const light = page.getByRole('button', { name: /Light mode/ });
  if (await light.count()) await light.click();
  await delay(600);
}

async function stop(session) {
  await session.browser.close().catch(() => {});
  session.app.kill();
  await delay(800);
}

const scenarios = {
  async map() {
    const directory = path.join(root, '.tools', `docs-map-${randomUUID()}`); await mkdir(directory, { recursive: true });
    const s = await launch(directory); const { page } = s;
    try {
      const servers = [['web-01', 'web-01.example'], ['media-01', 'media-01.example'], ['nas-01', 'nas-01.example']];
      await page.waitForSelector('button.add-server');
      await classicLight(page);
      await record(page, 'map', async () => {
        await delay(900);
        for (const [name, host] of servers) {
          await click(page, page.locator('button.add-server'), 300);
          await typeInto(page, page.getByLabel('Server name'), name);
          await typeInto(page, page.getByLabel('Host or IP address'), host);
          await click(page, page.getByRole('button', { name: 'Save server', exact: true }), 700);
        }
        await click(page, page.getByRole('button', { name: 'Fit map', exact: true }), 900);
        for (const [from, to, label] of [['web-01', 'media-01', 'reverse proxy'], ['media-01', 'nas-01', 'backups']]) {
          await click(page, page.getByRole('button', { name: /Document relationship/ }), 400);
          await page.locator('select[name=source]').selectOption({ label: from }); await delay(300);
          await page.locator('select[name=target]').selectOption({ label: to }); await delay(300);
          await typeInto(page, page.locator('input[name=label]'), label);
          await click(page, page.getByRole('button', { name: 'Save relationship', exact: true }), 800);
        }
        await click(page, page.getByRole('button', { name: 'Fit map', exact: true }), 1500);
      });
    } finally { await page.screenshot({ path: path.join(directory, 'last.png') }).catch(() => {}); await stop(s); }
  },

  // Health checks against two throwaway loopback listeners: healthy, then the service is stopped.
  async health() {
    const directory = path.join(root, '.tools', `docs-health-${randomUUID()}`); await mkdir(directory, { recursive: true });
    await writeFile(path.join(directory, 'known_hosts'), '');
    await writeFile(path.join(directory, 'ssh_config'), `Host *\n  BatchMode yes\n  StrictHostKeyChecking yes\n  UserKnownHostsFile ${path.join(directory, 'known_hosts').replace(/\\/g, '/')}\n`);
    const listen = server => new Promise(resolve => server.listen(0, '127.0.0.1', () => resolve(server.address().port)));
    const hostServer = net.createServer(socket => socket.end());
    const serviceServer = http.createServer((_request, response) => { response.writeHead(200, { connection: 'close' }); response.end(); });
    const hostPort = await listen(hostServer), servicePort = await listen(serviceServer);
    const s = await launch(directory); const { page, invoke } = s;
    try {
      await invoke('add_server', { input: { name: 'loopback-demo', host: '127.0.0.1' } });
      await page.reload(); await page.waitForSelector('button.add-server'); await installCursor(page);
      await classicLight(page);
      await page.getByRole('button', { name: 'Open loopback-demo workspace', exact: true }).click();
      const dialog = page.getByRole('dialog', { name: 'loopback-demo', exact: true });
      await dialog.waitFor(); await delay(600);
      await dialog.getByRole('tab', { name: 'Health', exact: true }).click(); await delay(600);
      const health = dialog.locator('.health-panel');
      const states = health.locator('.health-results .state-pill');
      await record(page, 'health', async () => {
        await delay(900);
        await typeInto(page, health.locator('input[name="hostPort"]'), String(hostPort));
        await move(page, health.locator('select[name="kind"]')); await health.locator('select[name="kind"]').selectOption('http'); await delay(500);
        await typeInto(page, health.locator('input[name="servicePort"]'), String(servicePort));
        await typeInto(page, health.locator('input[name="path"]'), '/health');
        await click(page, health.getByRole('button', { name: 'Save & start checks', exact: true }), 300);
        await page.waitForFunction(() => [...document.querySelectorAll('.health-results .state-pill')].some(e => e.textContent === 'healthy'), null, { timeout: 20000 });
        await delay(2200);
        serviceServer.close(); serviceServer.closeAllConnections?.();
        await click(page, health.getByRole('button', { name: /Check now|Run checks|Refresh/i }).first(), 300).catch(() => {});
        await page.waitForFunction(() => [...document.querySelectorAll('.health-results .state-pill')].some(e => e.textContent === 'unhealthy'), null, { timeout: 30000 }).catch(() => {});
        await delay(1800);
      });
    } finally { await page.screenshot({ path: path.join(directory, 'last.png') }).catch(() => {}); await stop(s); hostServer.close(); serviceServer.close(); }
  },

  // Files over a disposable loopback SFTP server (scripts/ssh_fixture.py): strict host trust, throwaway key.
  async files() {
    const directory = path.join(root, '.tools', `docs-files-${randomUUID()}`); await mkdir(directory, { recursive: true });
    const fixture = spawn(path.join(root, '.tools/ssh-fixture-venv/Scripts/python.exe'), ['scripts/ssh_fixture.py', path.join(directory, 'fixture')], { cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'inherit'] });
    const info = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('fixture startup timed out')), 60000);
      createInterface({ input: fixture.stdout }).once('line', line => { clearTimeout(timer); resolve(JSON.parse(line)); });
    });
    const s = await launch(directory, { sshConfig: info.config }); const { page, invoke } = s;
    try {
      await invoke('add_server', { input: { name: 'files-demo', host: 'atlas-trusted' } });
      await page.reload(); await page.waitForSelector('button.add-server'); await installCursor(page);
      await classicLight(page);
      await page.getByRole('button', { name: 'Open files-demo workspace', exact: true }).click();
      const dialog = page.getByRole('dialog', { name: 'files-demo', exact: true });
      await dialog.waitFor(); await delay(800);
      await record(page, 'files', async () => {
        await delay(700);
        await click(page, dialog.getByRole('tab', { name: 'Files', exact: true }));
        await click(page, dialog.getByRole('button', { name: 'Open file session', exact: true }), 1400);
        await click(page, dialog.getByRole('button', { name: 'hello.txt', exact: true }), 1500);
        await click(page, dialog.getByRole('button', { name: 'Open folder deep', exact: true }), 1400);
        await click(page, dialog.getByRole('navigation', { name: 'Remote directory' }).getByRole('button', { name: 'fixture', exact: true }), 1000);
        await click(page, dialog.getByRole('button', { name: 'Lock session', exact: true }), 1600);
      });
    } finally { await page.screenshot({ path: path.join(directory, 'last.png') }).catch(() => {}); await stop(s); fixture.stdin.end('quit\n'); await delay(1500); fixture.kill(); }
  },
};

const wanted = process.argv[2] ?? 'all';
for (const name of wanted === 'all' ? Object.keys(scenarios) : [wanted]) {
  if (!scenarios[name]) { console.error(`unknown scenario ${name}`); process.exit(1); }
  await scenarios[name]();
}
