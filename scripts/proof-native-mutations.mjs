import { expect } from '@playwright/test';
import { readFile, access } from 'node:fs/promises';
import path from 'node:path';
const exists = file => access(file).then(() => true, () => false);
export async function runMutationJourney({ page, invoke, rejection, before, info, directory }) {
  const actual = path.join(info.directory, 'filesystem/home/fixture/write-area');
  const trusted = before.servers.find(server => server.host === 'atlas-trusted');
  const changed = before.servers.find(server => server.host === 'atlas-changed');
  const source = await readFile(path.join(actual, 'source file.txt'));
  await page.getByRole('button', { name: 'Open trusted fixture workspace', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'trusted fixture', exact: true });
  await dialog.getByRole('tab', { name: 'Files', exact: true }).click();
  const panel = dialog.getByRole('form', { name: 'Review file change' });
  const buttons = name => dialog.getByRole('button', { name, exact: true });
  async function connect() { await buttons('Open file session').click(); await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture'); await buttons('Open folder write-area').click(); await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture/write-area'); }
  async function select(name) { await buttons(name).click(); await expect(dialog.locator('.file-details h4')).toHaveText(name); }
  async function review() { await panel.getByRole('button', { name: 'Review change', exact: true }).click(); await expect(panel.getByText('Server', { exact: true })).toBeVisible(); await expect(panel).toContainText('atlas-trusted'); await expect(panel).toContainText('fixture@127.0.0.1'); }
  async function confirm(deletion = false) { await panel.getByRole('button', { name: deletion ? 'Confirm delete' : 'Confirm change', exact: true }).click(); await expect(panel).toHaveCount(0); await expect(dialog.locator('.file-list')).toBeVisible(); }
  async function work() { await dialog.getByRole('navigation', { name: 'Remote directory' }).getByRole('button', { name: 'write-area', exact: true }).click(); await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture/write-area'); }
  await connect();
  await expect(buttons('odd-\\xff-source')).toBeVisible(); await expect(buttons('odd-\\\\xff-source')).toBeVisible();
  await buttons('New folder').click(); await panel.getByRole('textbox', { name: 'New folder name', exact: true }).fill('cancelled 日本語'); await review();
  expect(await exists(path.join(actual, 'cancelled 日本語'))).toBe(false); await panel.getByRole('button', { name: 'Cancel change', exact: true }).click();
  expect(await exists(path.join(actual, 'cancelled 日本語'))).toBe(false);
  await buttons('New folder').click(); await panel.getByRole('textbox', { name: 'New folder name', exact: true }).fill('new 日本語'); await review(); await confirm();
  await expect(buttons('new 日本語')).toBeVisible(); expect(await exists(path.join(actual, 'new 日本語'))).toBe(true);
  await buttons('New folder').click(); await panel.getByRole('textbox', { name: 'New folder name', exact: true }).fill('slow-confirmed'); await review();
  await panel.getByRole('button', { name: 'Confirm change', exact: true }).click();
  await expect(dialog.getByRole('tab', { name: 'Services', exact: true })).toBeDisabled();
  await page.keyboard.press('Escape'); await expect(dialog).toBeVisible();
  await expect(panel).toHaveCount(0); await expect(buttons('slow-confirmed')).toBeVisible();
  await select('slow-confirmed'); await buttons('Delete selected entry').click(); await review(); await confirm(true);
  await select('source file.txt'); await buttons('Rename selected entry').click(); await panel.getByRole('textbox', { name: 'New name', exact: true }).fill('renamed spaced.txt'); await review();
  await expect(panel).toContainText('/home/fixture/write-area/source file.txt'); await expect(panel).toContainText('/home/fixture/write-area/renamed spaced.txt'); await confirm();
  await expect(buttons('renamed spaced.txt')).toBeVisible(); expect(await readFile(path.join(actual, 'renamed spaced.txt'))).toEqual(source);
  await select('renamed spaced.txt'); await buttons('Rename selected entry').click(); await panel.getByRole('textbox', { name: 'New name', exact: true }).fill('existing.txt');
  await panel.getByRole('button', { name: 'Review change', exact: true }).click(); await expect(dialog.getByRole('alert')).toContainText('exists or changed'); await panel.getByRole('button', { name: 'Cancel change', exact: true }).click();
  await buttons('Move selected entry').click(); await expect(dialog.locator('.file-move-choice')).toContainText('/home/fixture/write-area/renamed spaced.txt');
  await buttons('Cancel move').click(); await expect(dialog.locator('.file-move-choice')).toHaveCount(0); expect(await exists(path.join(actual, 'renamed spaced.txt'))).toBe(true);
  await select('renamed spaced.txt'); await buttons('Move selected entry').click(); await buttons('Open folder target').click(); await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture/write-area/target');
  await buttons('Move here').click(); await review(); await expect(panel).toContainText('/home/fixture/write-area/target/renamed spaced.txt'); await confirm();
  expect(await readFile(path.join(actual, 'target/renamed spaced.txt'))).toEqual(source); expect(await exists(path.join(actual, 'renamed spaced.txt'))).toBe(false);
  await select('renamed spaced.txt'); await buttons('Delete selected entry').click(); await review();
  await page.screenshot({ path: path.join(directory, 'native-delete-preview.png') });
  await panel.getByRole('button', { name: 'Cancel change', exact: true }).click(); expect(await exists(path.join(actual, 'target/renamed spaced.txt'))).toBe(true);
  await buttons('Delete selected entry').click(); await review(); await confirm(true);
  expect(await exists(path.join(actual, 'target/renamed spaced.txt'))).toBe(false); await expect(buttons('renamed spaced.txt')).toHaveCount(0);
  await work(); await select('folder-link'); await buttons('Delete selected entry').click(); await review(); await expect(panel).toContainText('Source (symlink)'); await confirm(true);
  await expect(buttons('folder-link')).toHaveCount(0); expect(await readFile(path.join(actual, 'target/keep.txt'), 'utf8')).toBe('symlink target survives');
  await select('odd-\\xff-source'); await buttons('Delete selected entry').click(); await review();
  await expect(panel).toContainText('/home/fixture/write-area/odd-\\xff-source'); await confirm(true);
  await expect(buttons('odd-\\xff-source')).toHaveCount(0); await expect(buttons('odd-\\\\xff-source')).toBeVisible();
  await select('nonempty'); await buttons('Delete selected entry').click(); await review(); await confirm(true);
  await expect(dialog.getByRole('alert')).toContainText('server rejected'); expect(await readFile(path.join(actual, 'nonempty/keep.txt'), 'utf8')).toBe('never recurse');
  await buttons('Open folder denied').click(); await expect(dialog.locator('.file-location code')).toHaveText('/home/fixture/write-area/denied');
  await buttons('New folder').click(); await panel.getByRole('textbox', { name: 'New folder name', exact: true }).fill('no-permission'); await review(); await confirm();
  await expect(dialog.getByRole('alert')).toContainText('cannot access'); expect(await exists(path.join(actual, 'denied/no-permission'))).toBe(false);
  await work(); await select('new 日本語'); await buttons('Delete selected entry').click(); await review(); await confirm(true); expect(await exists(path.join(actual, 'new 日本語'))).toBe(false);
  await page.screenshot({ path: path.join(directory, 'native-mutations-reconciled.png') });
  await buttons('New folder').click(); await panel.getByRole('textbox', { name: 'New folder name', exact: true }).fill('locked-preview'); await review();
  await buttons('Lock session').click(); await expect(panel).toHaveCount(0); await expect(dialog.locator('.file-list')).toHaveCount(0); expect(await exists(path.join(actual, 'locked-preview'))).toBe(false);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(0); await page.keyboard.press('Escape');

  // Native negative calls use actual backend grants, never mocked transports.
  const ticket = await invoke('files_prepare', { serverId: trusted.id });
  const session = await invoke('files_open', { serverId: trusted.id, operationId: ticket.operation_id });
  const home = await invoke('files_read', { serverId: trusted.id, sessionId: session.session_id, action: { kind: 'list', resource_id: session.home.id, cursor: null } });
  const area = home.page.entries.find(entry => entry.resource.label === 'write-area');
  const current = await invoke('files_read', { serverId: trusted.id, sessionId: session.session_id, action: { kind: 'list', resource_id: area.resource.id, cursor: null } });
  const args = { serverId: trusted.id, sessionId: session.session_id };
  const change = { kind: 'mkdir', parent_id: current.page.directory.id, name: 'wrong-host-never-created' };
  const preview = await invoke('files_preview', { ...args, change });
  expect((await rejection('files_commit', { ...args, serverId: changed.id, operationId: preview.operation_id })).code).toBe('denied');
  await invoke('files_discard', { ...args, operationId: preview.operation_id });
  expect((await rejection('files_commit', { ...args, operationId: preview.operation_id })).code).toBe('not_found');
  expect(await exists(path.join(actual, 'wrong-host-never-created'))).toBe(false);
  expect((await rejection('files_preview', { ...args, change: { ...change, parent_id: '/home/fixture' } })).code).toBe('denied');
  const extra = await rejection('files_preview', { ...args, change: { ...change, overwrite: true } }); expect(typeof extra === 'string' ? extra : extra.code).toMatch(/unknown|invalid|denied/i);
  const stale = await invoke('files_preview', { ...args, change: { ...change, name: 'stale-never-created' } });
  await invoke('files_read', { ...args, action: { kind: 'list', resource_id: current.page.directory.id, cursor: null } });
  expect((await rejection('files_commit', { ...args, operationId: stale.operation_id })).code).toBe('not_found');
  await invoke('files_lock', { sessionId: session.session_id });
  expect((await rejection('files_commit', { ...args, operationId: stale.operation_id })).code).toBe('expired_session');

  await page.getByRole('button', { name: 'Open trusted fixture workspace', exact: true }).click();
  await dialog.getByRole('tab', { name: 'Files', exact: true }).click(); await connect();
  await buttons('New folder').click(); await panel.getByRole('textbox', { name: 'New folder name', exact: true }).fill('apply-then-disconnect'); await review();
  await panel.getByRole('button', { name: 'Confirm change', exact: true }).click();
  await expect(dialog.locator('.file-list')).toHaveCount(0); await expect(dialog.locator('.file-session')).toHaveCount(0);
  await expect(dialog).toContainText('may have applied'); expect(await exists(path.join(actual, 'apply-then-disconnect'))).toBe(true);
  await expect.poll(async () => (await invoke('workspace_status')).pending_operations).toBe(0);
  const finalStats = JSON.parse(await readFile(info.stats, 'utf8')); expect(finalStats.trusted.mutations).toBe(11);
  expect(await readFile(path.join(actual, 'existing.txt'), 'utf8')).toBe('fixture preserve existing.txt');
  await page.screenshot({ path: path.join(directory, 'native-mutation-outcome-unknown.png') }); await page.keyboard.press('Escape');
  return ['native create/rename/move/delete previews and explicit confirmations, cancel makes no change, real refresh reconciles results', 'collision/permission/nonempty-directory rejection, symlink target preserved, pinned source survives destination navigation', 'native wrong-host/forged-resource/extra-field/stale/cancelled/locked mutation tickets denied', 'lost acknowledgement clears session, reports uncertain outcome and never retries the applied change'];
}
