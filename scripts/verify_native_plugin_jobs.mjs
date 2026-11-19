// Actual Windows Tauri/WebView2 proof for plugin jobs (F24/T049, owner
// decision D061). A sandboxed job asks InfraWerf to restart a user service
// and to start a system service that needs sudo; InfraWerf runs each only as
// the owner's trust level, exceptions, schedules and dialog allow. Real
// workload controls run in the disposable Linux container over real OpenSSH.
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root = process.cwd();
const directory = path.join(root, '.tools', `native-plugin-jobs-proof-${randomUUID()}`);
await mkdir(directory, { recursive: false });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const plugins = path.join(directory, 'plugins');
await cp(path.join(root, 'scripts/fixtures/job-plugin'), path.join(plugins, 'job-plugin'), { recursive: true });
await cp(path.join(root, 'examples/service-summary'), path.join(plugins, 'service-summary'), { recursive: true });
const KEEPER = 'org.example.service-keeper', SUMMARY = 'org.example.service-summary';
// One schedule "minute" is two seconds here, so a 5-minute schedule fires every 10 s.
const MINUTE_SECONDS = 2;

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
  const inContainer = (...args) => spawnSync('podman', ['--connection', 'podman-machine-default', 'exec', info.container, ...args], { encoding: 'utf8', windowsHide: true, timeout: 30000 });
  // Workload controls the fixture's systemctl actually carried out.
  const controls = () => Number(inContainer('cat', '/tmp/atlas-control-count').stdout.trim() || 0);
  const serviceState = (scope, name) => inContainer('cat', `/tmp/atlas-control-${scope}-${name}`).stdout.trim() || 'inactive';
  const database = path.join(directory, 'native.sqlite3');
  const { ATLAS_LEGACY_DB, ATLAS_REPORTS_DIR, ...environment } = process.env;
  app = spawn(process.env.ATLAS_DESKTOP_BINARY ? path.resolve(process.env.ATLAS_DESKTOP_BINARY) : path.join(root, 'target/debug/atlas-native.exe'), [], {
    cwd: root, windowsHide: true,
    env: { ...environment, ATLAS_NATIVE_DB: database, ATLAS_SSH_CONFIG: info.config, ATLAS_PLUGINS_DIR: plugins, ATLAS_SCHEDULE_MINUTE_SECONDS: String(MINUTE_SECONDS), WEBVIEW2_USER_DATA_FOLDER: path.join(directory, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9243 --remote-debugging-address=127.0.0.1' },
    stdio: 'ignore',
  });
  for (let i = 0; i < 100; i++) {
    if (app.exitCode !== null) throw new Error('Native app exited during startup');
    try { if ((await fetch('http://127.0.0.1:9243/json/version')).ok) break; } catch {}
    await delay(200);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9243');
  page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForLoadState('domcontentloaded');
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  const log = async () => (await invoke('plugin_activity', { limit: 500 })).entries;
  await expect(page.getByText('A clear place to start.', { exact: true })).toBeVisible();
  await invoke('add_server', { input: { name: 'Job host', host: 'atlas-linux' } });
  await invoke('add_server', { input: { name: 'Other host', host: 'other-job.home' } });
  const servers = (await invoke('map_state')).servers;
  const host = servers.find(s => s.host === 'atlas-linux'), other = servers.find(s => s.host !== 'atlas-linux');
  const jobs = async (serverId = host.id) => invoke('plugin_jobs', { serverId });
  const latest = async () => (await jobs()).runs.find(r => r.plugin_id === KEEPER);
  const finished = async (after, attended) => {
    let run;
    await expect.poll(async () => { run = await latest(); return run && run.run_id !== after && run.attended === attended && run.state !== 'running' ? run.state : 'waiting'; }, { timeout: 60000 }).not.toBe('waiting');
    return run;
  };
  const outcomes = run => run.actions.map(a => `${a.action_id}:${a.outcome}${a.outcome === 'refused' ? `:${a.message}` : ''}`);

  // 1. Nothing until enabled and allowed; then only on that server.
  expect((await jobs()).jobs).toEqual([]);
  await invoke('plugin_enable', { pluginId: KEEPER, trust: 'confirm_changes' });
  await invoke('plugin_set_grant', { pluginId: KEEPER, serverId: host.id, capabilities: ['read_saved_state', 'ssh_session'] });
  expect((await jobs()).jobs.map(j => j.title)).toEqual(['Keep services up']);
  expect((await jobs(other.id)).jobs).toEqual([]);
  results.push('the job appears only after the plugin is enabled and allowed to read on a server, and only on that server');

  // 2. The plugin's page shows what the job may do, and the owner's settings per server.
  const nav = page.getByRole('navigation', { name: 'Main navigation' });
  await nav.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('navigation', { name: 'Settings pages' }).getByRole('button', { name: 'Plugins', exact: true }).click();
  const section = page.getByRole('region', { name: 'Plugins', exact: true });
  await section.getByRole('button', { name: 'Configure Service keeper', exact: true }).click();
  const pluginPage = page.getByRole('region', { name: 'Service keeper plugin', exact: true });
  const consent = pluginPage.getByRole('list', { name: 'Actions it may ask for' });
  await expect(consent).toContainText('Restart the worker: Restart a service from that server’s inventory. Restarts fixture-worker.service so it picks up new work.');
  await expect(consent).toContainText('Start the stopped service: Start a service from that server’s inventory. Starts fixture-stopped.service, a system service, which needs sudo.');
  const settings = pluginPage.getByLabel('Jobs on Job host', { exact: true });
  await expect(settings).toContainText('No schedule: runs only when you press Run');
  const exception = settings.getByLabel('Allow sudo for “Start the stopped service” on Job host (asks your password every time)');
  await expect(exception).not.toBeChecked();
  await expect(pluginPage.getByLabel('Jobs on Other host', { exact: true })).toHaveCount(0);
  await page.screenshot({ path: path.join(directory, 'plugin-page.png'), fullPage: true });
  results.push('the plugin page lists each action the job may ask for with its control, target and effect before enabling, and the schedule and per-action sudo exception for each server where the job may run');

  // 3. Open the workspace's Plugin data tab.
  await page.reload();
  await page.getByRole('button', { name: 'Open Job host workspace', exact: true }).click();
  const workspace = page.getByRole('dialog', { name: 'Job host', exact: true });
  await workspace.getByRole('button', { name: 'Refresh inventory', exact: true }).click();
  await expect(workspace.getByRole('status')).toContainText('Approved inventory updated', { timeout: 60000 });
  await workspace.getByRole('tab', { name: 'Plugin data', exact: true }).click();
  const card = workspace.getByRole('article', { name: 'Keep services up job' });
  const runNow = card.getByRole('button', { name: 'Run Keep services up', exact: true });
  const ask = page.getByRole('dialog', { name: 'A plugin job asks to make a change', exact: true });

  // 4. Without control_workloads every action is refused and nothing runs.
  let before = controls();
  await runNow.click();
  let run = await finished(null, true);
  expect(run.state).toBe('done');
  expect(outcomes(run)).toEqual(['restart-worker:refused:not_granted', 'start-stopped:refused:not_granted']);
  await expect(card.getByLabel('Keep services up log')).toContainText('service keeper: checking workloads');
  await expect(card.getByLabel('Keep services up log')).toContainText('"error":"not_granted"');
  expect(controls()).toBe(before);
  results.push('without "control workloads" allowed on the server, both actions were refused and nothing ran; the job carried on and its log shows the refusals');

  // 5. Allowed, at confirm-changes: InfraWerf asks first. Sudo needs an exception.
  await invoke('plugin_set_grant', { pluginId: KEEPER, serverId: host.id, capabilities: ['read_saved_state', 'ssh_session', 'control_workloads'] });
  await inContainer('rm', '-f', '/tmp/atlas-control-user-fixture-worker.service');
  before = controls();
  await runNow.click();
  await expect(ask).toBeVisible({ timeout: 30000 });
  await expect(ask).toContainText('Service keeper’s job Keep services up asks to restart a service fixture-worker.service on Job host.');
  await expect(ask).toContainText('Restart the worker: Restarts fixture-worker.service so it picks up new work.');
  await expect(ask.getByRole('button', { name: 'Run (wait…)', exact: true })).toBeDisabled();
  await expect(ask.getByRole('button', { name: 'Decline', exact: true })).toBeFocused();
  await expect(ask.getByLabel('Sudo password for this action')).toHaveCount(0);
  await page.screenshot({ path: path.join(directory, 'job-asks.png') });
  expect(controls()).toBe(before);
  await ask.getByRole('button', { name: 'Run', exact: true }).click();
  run = await finished(run.run_id, true);
  expect(outcomes(run)).toEqual(['restart-worker:done', 'start-stopped:refused:no_elevation_exception']);
  expect(controls()).toBe(before + 1);
  expect(serviceState('user', 'fixture-worker.service')).toBe('active');
  expect(serviceState('system', 'fixture-stopped.service')).toBe('inactive');
  results.push('at "changes with confirmation", the restart waited in InfraWerf\'s own dialog (Run disabled at first, Decline focused, no password field for a user service) and ran only after Run, leaving fixture-worker.service active; the sudo action was refused for want of an exception, before any dialog');

  // 6. Declining runs nothing.
  before = controls();
  await runNow.click();
  await expect(ask).toBeVisible({ timeout: 30000 });
  await ask.getByRole('button', { name: 'Decline', exact: true }).click();
  run = await finished(run.run_id, true);
  expect(outcomes(run)).toEqual(['restart-worker:refused:declined', 'start-stopped:refused:no_elevation_exception']);
  expect(controls()).toBe(before);
  results.push('declining in the dialog ran nothing');

  // 7. With the owner's exception, sudo needs the password, every time, and it is never stored.
  await invoke('plugin_set_elevation', { serverId: host.id, pluginId: KEEPER, actionId: 'start-stopped', granted: true });
  before = controls();
  await runNow.click();
  await expect(ask).toContainText('restart a service fixture-worker.service', { timeout: 30000 });
  await ask.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(ask).toContainText('start a service fixture-stopped.service on Job host', { timeout: 30000 });
  const password = ask.getByLabel('Sudo password for this action');
  await expect(password).toBeVisible();
  await expect(ask.getByRole('button', { name: /^Run/ })).toBeDisabled();
  await page.screenshot({ path: path.join(directory, 'job-asks-sudo.png') });
  await delay(1100);
  await expect(ask.getByRole('button', { name: 'Run', exact: true })).toBeDisabled();
  await password.fill(info.password);
  await ask.getByRole('button', { name: 'Run', exact: true }).click();
  run = await finished(run.run_id, true);
  expect(outcomes(run)).toEqual(['restart-worker:done', 'start-stopped:done']);
  expect(controls()).toBe(before + 2);
  expect(serviceState('system', 'fixture-stopped.service')).toBe('active');
  const stored = [];
  for (const suffix of ['', '-wal']) { try { stored.push(await readFile(database + suffix)); } catch {} }
  expect(Buffer.concat(stored).includes(Buffer.from(info.password)), 'sudo password in the database').toBe(false);
  expect(JSON.stringify(await log())).not.toContain(info.password);
  expect(JSON.stringify(await jobs())).not.toContain(info.password);
  results.push('with the owner\'s exception for that one action, the sudo action asked for the password in InfraWerf\'s dialog, Run stayed disabled until it was typed, and fixture-stopped.service then started with fresh sudo; the password is in neither the database, the activity log nor the job\'s state');

  // 8. A scheduled run below trusted automation changes nothing, and never asks.
  await inContainer('rm', '-f', '/tmp/atlas-control-system-fixture-stopped.service');
  await invoke('plugin_set_schedule', { serverId: host.id, pluginId: KEEPER, contributionId: 'keeper', everyMinutes: 5 });
  before = controls();
  run = await finished(run.run_id, false);
  expect(outcomes(run)).toEqual(['restart-worker:refused:needs_confirmation', 'start-stopped:refused:sudo_never_unattended']);
  await expect(ask).toHaveCount(0);
  expect(controls()).toBe(before);
  await expect(card).toContainText('by its schedule');
  await expect(card).toContainText('Every 5 minutes while InfraWerf is open');
  results.push(`a 5-minute schedule (a minute is ${MINUTE_SECONDS} s in this test) ran the job unattended at "changes with confirmation": both actions were refused and logged, no dialog appeared, and nothing ran`);

  // 9. Trusted automation: the user-service action runs unattended; sudo never does.
  await invoke('plugin_enable', { pluginId: KEEPER, trust: 'trusted_automation' });
  await invoke('plugin_set_schedule', { serverId: host.id, pluginId: KEEPER, contributionId: 'keeper', everyMinutes: 5 });
  before = controls();
  run = await finished(run.run_id, false);
  expect(outcomes(run)).toEqual(['restart-worker:done', 'start-stopped:refused:sudo_never_unattended']);
  await expect(ask).toHaveCount(0);
  expect(controls()).toBe(before + 1);
  expect(serviceState('system', 'fixture-stopped.service')).toBe('inactive');
  results.push('at trusted automation the scheduled run restarted fixture-worker.service without asking; the sudo action was still refused, as sudo never runs unattended even with an exception');

  // 10. Withdrawing trusted automation deletes the schedule.
  await invoke('plugin_revoke_trusted', { pluginId: KEEPER });
  expect((await jobs()).jobs[0].schedule).toBe(null);
  const lastScheduled = (await latest()).run_id;
  await delay(MINUTE_SECONDS * 5 * 1000 + 3000);
  expect((await latest()).run_id).toBe(lastScheduled);
  results.push('withdrawing trusted automation deleted the schedule, and no further run started');

  // 11. A request waiting when plugins are turned off runs nothing.
  before = controls();
  await runNow.click();
  await expect(ask).toBeVisible({ timeout: 30000 });
  await invoke('plugins_set_all_off', { off: true });
  run = await finished(run.run_id, true);
  expect(run.state).toBe('cancelled');
  await ask.getByRole('button', { name: 'Run', exact: true }).click();
  await expect(ask).toHaveCount(0);
  expect(controls()).toBe(before);
  expect((await jobs()).jobs).toEqual([]);
  await invoke('plugins_set_all_off', { off: false });
  results.push('turning all plugins off cancelled a run waiting in the dialog: pressing Run afterwards ran nothing, and no job was offered while plugins were off');

  // 12. Another plugin's change leaves this run alone; the example job reads
  // the workloads and returns them.
  await runNow.click();
  await expect(ask).toBeVisible({ timeout: 30000 });
  const waiting = (await latest()).run_id;
  await invoke('plugin_enable', { pluginId: SUMMARY, trust: 'read_only' });
  await invoke('plugin_set_grant', { pluginId: SUMMARY, serverId: host.id, capabilities: ['read_saved_state'] });
  await delay(1000);
  expect((await latest()).state).toBe('running');
  await ask.getByRole('button', { name: 'Decline', exact: true }).click();
  // The sudo exception from step 7 still stands, so the second action asks too.
  await expect(ask).toContainText('fixture-stopped.service', { timeout: 30000 });
  await ask.getByRole('button', { name: 'Decline', exact: true }).click();
  run = await finished(run.run_id, true);
  expect(run.run_id).toBe(waiting);
  expect(outcomes(run)).toEqual(['restart-worker:refused:declined', 'start-stopped:refused:declined']);
  results.push('enabling and allowing a different plugin did not cancel the run waiting in the dialog: job runs stop only for the plugin whose permissions changed');
  const summary = workspace.getByRole('article', { name: 'Service summary job' });
  await summary.getByRole('button', { name: 'Run Service summary', exact: true }).click();
  await expect(summary.getByLabel('Service summary last run')).toContainText('Finished', { timeout: 30000 });
  const summaryRun = (await jobs()).runs.find(r => r.plugin_id === SUMMARY);
  const names = summaryRun.outcome.result.ok.map(w => w.name);
  expect(names).toContain('fixture-worker.service');
  expect(names).toContain('fixture-stopped.service');
  await page.screenshot({ path: path.join(directory, 'plugin-data-jobs.png') });
  results.push(`the example job read the server's ${names.length} saved workloads through the sandbox and returned them, at read-only trust`);

  // 13. Everything is in the plugin activity log.
  const entries = await log();
  await writeFile(path.join(directory, 'activity.json'), JSON.stringify(entries, null, 2));
  const keeper = entries.filter(e => e.plugin_id === KEEPER);
  for (const [action, outcome, detail] of [
    ['job_run', 'done', /^keeper: done$/],
    ['job_run', 'done', /^keeper: scheduled, done$/],
    ['job_run', 'cancelled', /^keeper: cancelled$/],
    ['job_action', 'done', /^restart-worker: fixture-worker\.service: completed$/],
    ['job_action', 'done', /^start-stopped: fixture-stopped\.service: completed$/],
    ['job_action', 'needs_confirmation', /^start-stopped: fixture-stopped\.service: asked you$/],
    ['job_action', 'refused', /^restart-worker: fixture-worker\.service: declined$/],
    ['job_action', 'refused', /^start-stopped: fixture-stopped\.service: sudo_never_unattended$/],
    ['job_action', 'refused', /^restart-worker: : not_granted$/],
    ['set_schedule', 'done', /^keeper: every 5 minutes$/],
    ['elevation_exception', 'done', /^start-stopped: granted$/],
  ]) expect(keeper.some(e => e.action === action && e.outcome === outcome && detail.test(e.detail)), `${action} ${outcome} ${detail} in ${JSON.stringify(keeper.map(e => [e.action, e.outcome, e.detail]))}`).toBe(true);
  results.push('each run, action (done, declined or refused with its reason), schedule and exception is in the plugin activity log');

  expect(pageErrors).toEqual([]);
  const proof = { result: 'pass', results, targets: 'disposable loopback Linux container only', noRealHost: true, sudoOnlyInContainer: true, pageErrors, directory };
  await writeFile(path.join(directory, 'result.json'), JSON.stringify(proof, null, 2));
  console.log(JSON.stringify(proof, null, 2));
  await workspace.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Quit InfraWerf' }).click();
  await expect.poll(() => app.exitCode).not.toBe(null);
} catch (failure) {
  console.error(`Native plugin jobs proof failed in ${directory}.`);
  await page?.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
  throw failure;
} finally {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) { app.kill(); await new Promise(resolve => app.once('exit', resolve)); }
  if (server.exitCode === null) {
    server.stdin.end('\n');
    await Promise.race([new Promise(resolve => server.once('exit', resolve)), delay(60000)]);
    if (server.exitCode === null) server.kill();
  }
  lines.close();
}
