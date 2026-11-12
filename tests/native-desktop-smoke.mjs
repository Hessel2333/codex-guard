import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

// A real Tauri WebView + real installed bundled CLI, but a temporary CODEX_HOME.
// No confirmed write/restore is issued here. Full transactions are exercised by
// the Rust integration test, which also isolates Guard's snapshot directory.
const artifacts = process.env.GUARD_DESKTOP_ARTIFACTS || path.join(os.tmpdir(), `codex-guard-native-desktop-${process.pid}`);
await mkdir(artifacts, { recursive: true });
const home = path.join(artifacts, 'codex-home');
await mkdir(home);
const config = path.join(home, 'config.toml');
const original = 'developer_instructions = "原生测试工作偏好"\nmodel_reasoning_effort = "low"\napproval_policy = "on-request"\nsandbox_mode = "read-only"\n';
await writeFile(config, original);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const realHome = process.env.CODEX_HOME || path.join(os.homedir(), '.codex');
const originalUserHashes = {};
for (const name of ['config.toml', 'AGENTS.md']) {
  try { originalUserHashes[name] = hash(await readFile(path.join(realHome, name))); }
  catch (e) { if (e.code !== 'ENOENT') throw e; originalUserHashes[name] = null; }
}
const exe = path.resolve(process.env.GUARD_EXE || 'src-tauri/target/debug/codex-guard.exe');
const child = spawn(exe, [], { windowsHide: true, stdio: 'ignore', env: {
  ...process.env, CODEX_HOME: home,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19329',
  WEBVIEW2_USER_DATA_FOLDER: path.join(artifacts, 'webview'),
} });
let browser;
try {
  const deadline = Date.now() + 30000;
  while (!browser && Date.now() < deadline) {
    try { browser = await chromium.connectOverCDP('http://127.0.0.1:19329'); }
    catch { if (child.exitCode !== null) throw Error(`Desktop exited: ${child.exitCode}`); await new Promise(r => setTimeout(r, 300)); }
  }
  if (!browser) throw Error('Tauri WebView2 did not start');
  const context = browser.contexts()[0];
  let page;
  for (let i = 0; i < 60; i++) {
    page = context.pages().find(p => p.url().includes('tauri.localhost'));
    if (page) break;
    await new Promise(r => setTimeout(r, 250));
  }
  if (!page) throw Error('Native page not found');
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  await expect(page.getByRole('heading', { name: 'Codex Guard', exact: true })).toBeVisible();
  await page.evaluate(() => { localStorage.setItem('guard.autoUpdateCheck', 'false'); });
  const notes = page.getByRole('button', { name: '知道了', exact: true });
  if (await notes.isVisible()) await notes.click();
  await page.getByRole('button', { name: '桌面配置', exact: true }).click();
  await expect(page.getByLabel('自定义指令', { exact: true })).toBeEnabled({ timeout: 20000 });
  const invoke = (name, args) => page.evaluate(({ name, args }) => window.__TAURI_INTERNALS__.invoke(name, args), { name, args });
  const selection = { home: null };
  const status = await invoke('get_desktop_config_status', { selection });
  const normalize = value => path.win32.normalize(value.replace(/^\\\\\?\\/, '')).toLowerCase();
  expect(normalize(status.home)).toBe(normalize(home));
  expect(status.target.cli_path).toMatch(/resources[\\/]codex\.exe$/i);
  expect(status.home_runtime_verified).toBe(false);
  await expect(page.getByRole('checkbox', { name: '使用已保存的代理', exact: true })).toBeChecked();
  await page.getByRole('button', { name: '启动此配置', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('同时使用已保存的代理');
  await expect(page.getByRole('dialog')).toContainText('本次不会修改或移除现有指令配置');
  await page.getByRole('button', { name: '取消', exact: true }).click();
  const launchRejected = await page.evaluate(async ({ home }) => {
    try { await window.__TAURI_INTERNALS__.invoke('launch_desktop_config', { request: { selection: { home }, revision: '', target_identity: '', use_proxy: true, confirmed: false } }); return null; } catch (error) { return error; }
  }, { home });
  expect(launchRejected.code).toBe('CONFIRMATION_REQUIRED');
  await page.getByRole('button', { name: '检查兼容性', exact: true }).click();
  await expect(page.getByText('常规配置项已通过隔离检查')).toBeVisible({ timeout: 25000 });
  await expect(page.getByText('尚未验证；不能据此判断云端任务已生效')).toBeVisible();
  await page.getByLabel('自定义指令', { exact: true }).fill('仅用于 Codex Guard 原生预览测试');
  await page.getByRole('button', { name: '预览变更', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('原生测试工作偏好');
  await expect(page.getByRole('dialog')).toContainText('仅用于 Codex Guard 原生预览测试');
  await page.getByRole('button', { name: '取消', exact: true }).click();
  expect(await readFile(config, 'utf8')).toBe(original);
  for (const name of ['apply_desktop_config', 'restore_desktop_config']) {
    const request = name === 'apply_desktop_config'
      ? { selection, revision: '', target_identity: '', edits: [], confirmed: false }
      : { selection, revision: '', target_identity: '', snapshot_id: '123-1-1', confirmed: false };
    const rejected = await page.evaluate(async ({ name, request }) => {
      try { await window.__TAURI_INTERNALS__.invoke(name, { request }); return null; } catch (e) { return e; }
    }, { name, request });
    expect(rejected.code).toBe('CONFIRMATION_REQUIRED');
  }
  const stale = await page.evaluate(async ({ selection, revision }) => {
    try { await window.__TAURI_INTERNALS__.invoke('preview_desktop_config', { selection, revision, targetIdentity: 'not-the-selected-installation', edits: [{ key: 'developer_instructions', value: 'fixture' }] }); return null; }
    catch (e) { return e; }
  }, { selection, revision: status.revision });
  expect(stale.code).toBe('DESKTOP_TARGET_STALE');
  await page.getByRole('button', { name: '撤销未保存修改', exact: true }).click();
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('外观').selectOption('light'); await page.keyboard.press('Escape');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(artifacts, 'native-desktop-light.png'), fullPage: true });
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('外观').selectOption('dark'); await page.keyboard.press('Escape');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(artifacts, 'native-desktop-dark.png'), fullPage: true });
  await page.setViewportSize({ width: 680, height: 780 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await writeFile(config, '# >>> codex-purge managed block\n' + original);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByLabel('自定义指令', { exact: true })).toBeDisabled();
  await expect(page.getByRole('alert')).toContainText('正由 codex-purge 管理');
  await expect(page.getByRole('button', { name: '启动此配置', exact: true })).toBeEnabled();
  expect(errors).toEqual([]);
  for (const name of ['config.toml', 'AGENTS.md']) {
    let after = null;
    try { after = hash(await readFile(path.join(realHome, name))); } catch (e) { if (e.code !== 'ENOENT') throw e; }
    expect(after, `real user ${name} must remain unchanged`).toBe(originalUserHashes[name]);
  }
  const report = { result: 'passed', artifacts, instance: { executable: status.target.executable, cli: status.target.cli_path }, verified: ['native IPC', 'isolated bundled config/read', 'preview cancellation', 'backend confirmation guard', 'stale target guard', 'foreign manager read-only', 'light/dark', '680px layout', 'real user config and AGENTS hashes unchanged'], active_session_verified: false, model_turns_started: 0 };
  await writeFile(path.join(artifacts, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
} finally {
  if (browser) await browser.close();
  child.kill();
}
