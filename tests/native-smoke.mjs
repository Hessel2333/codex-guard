import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

// Test-only WebView2 remote debugging. Never enabled by application code.
const exe = path.resolve(process.env.GUARD_EXE || 'src-tauri/target/debug/codex-guard.exe');
const binary = await readFile(exe);
const peHeader = binary.readUInt32LE(0x3c);
expect(binary.readUInt16LE(peHeader + 24 + 68), 'Windows GUI subsystem prevents a startup console').toBe(2);
const artifacts = path.resolve('test-results');
await mkdir(artifacts, { recursive: true });
const child = spawn(exe, [], { windowsHide: true, stdio: 'ignore', env: {
  ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19327',
  WEBVIEW2_USER_DATA_FOLDER: path.join(artifacts, `webview-profile-${process.pid}`),
} });
let browser;
try {
  const deadline = Date.now() + 30000;
  while (!browser && Date.now() < deadline) {
    try { browser = await chromium.connectOverCDP('http://127.0.0.1:19327'); }
    catch { if (child.exitCode !== null) throw Error(`Desktop exited: ${child.exitCode}`); await new Promise(r => setTimeout(r, 300)); }
  }
  if (!browser) throw Error('Could not connect to the desktop WebView2');
  const context = browser.contexts()[0];
  let page;
  for (let i = 0; i < 60; i++) {
    page = context.pages().find(p => p.url().includes('tauri.localhost'));
    if (page) break;
    await new Promise(r => setTimeout(r, 250));
  }
  if (!page) throw Error('Tauri page did not load');
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  await expect(page.getByRole('heading', { name: 'Codex Guard', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled({ timeout: 20000 });
  const status = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_codex_status'));
  await writeFile(path.join(artifacts, 'native-status.json'), JSON.stringify(status, null, 2));

  const probe = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
    "$ErrorActionPreference='Stop'; $pkg=Get-AppxPackage OpenAI.Codex | Sort-Object Version -Descending | Select-Object -First 1; @{ version=if($pkg){$pkg.Version.ToString()}else{$null}; location=$pkg.InstallLocation; current=[Environment]::GetEnvironmentVariable('CODEX_CLI_PATH','User') } | ConvertTo-Json -Compress"], { encoding: 'utf8', windowsHide: true });
  if (probe.status !== 0) throw Error(probe.stderr);
  const reference = JSON.parse(probe.stdout.replace(/^\uFEFF/, ''));
  expect(status.version).toBe(reference.version);
  expect(status.install_location).toBe(reference.location);
  expect(status.current_cli_path).toBe(reference.current);
  if (status.installed) expect(status.expected_cli_path).toBe(path.win32.join(reference.location, 'app', 'resources', 'codex.exe'));
  const repairRejected = await page.evaluate(async () => {
    try { await window.__TAURI_INTERNALS__.invoke('repair_cli_path', { confirmed: false, expectedPath: '', previousPath: null }); return null; }
    catch (error) { return error; }
  });
  expect(repairRejected.code).toBe('CONFIRMATION_REQUIRED');
  const staleRepair = await page.evaluate(async () => {
    try { await window.__TAURI_INTERNALS__.invoke('repair_cli_path', { confirmed: true, expectedPath: '', previousPath: null }); return null; }
    catch (error) { return error; }
  });
  expect(['REPAIR_STALE', 'REPAIR_UNAVAILABLE']).toContain(staleRepair.code);
  const afterRejectedRepair = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_codex_status'));
  expect(afterRejectedRepair.current_cli_path).toBe(status.current_cli_path);

  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('外观').selectOption('light');
  await page.getByRole('button', { name: '关闭设置' }).click();
  await page.screenshot({ path: path.join(artifacts, 'native-overview.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '查看诊断' }).click();
  await expect(page.getByRole('heading', { name: '应用包' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '内置 CLI' })).toBeVisible();
  await page.screenshot({ path: path.join(artifacts, 'native-diagnostics.png'), fullPage: true });
  await page.getByRole('button', { name: '运行概览', exact: true }).click();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled({ timeout: 20000 });
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('外观').selectOption('dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.keyboard.press('Escape');
  await page.screenshot({ path: path.join(artifacts, 'native-dark.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '代理启动器', exact: true }).click();
  await expect(page.getByRole('heading', { name: '代理启动器', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '使用代理启动', exact: true })).toBeEnabled({ timeout: 15000 });
  const proxyStatus = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_proxy_status'));
  expect(proxyStatus.executable).toMatch(/(ChatGPT|Codex)\.exe$/i);
  // Exercise rejection paths without ending the Codex 会话 running this task.
  for (const command of ['launch_codex_proxy', 'stop_codex_proxy']) {
    const rejected = await page.evaluate(async name => {
      try { await window.__TAURI_INTERNALS__.invoke(name, { administrator: false, confirmed: false }); return null; }
      catch (error) { return error; }
    }, command);
    expect(rejected.code).toBe('CONFIRMATION_REQUIRED');
  }
  await page.getByRole('button', { name: '管理员启动', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('Windows 将通过 UAC 请求管理员授权');
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await page.screenshot({ path: path.join(artifacts, 'native-proxy-dark.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('外观').selectOption('light');
  await page.keyboard.press('Escape');
  await page.screenshot({ path: path.join(artifacts, 'native-proxy.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '运行概览', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('外观').selectOption('system');
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
  console.log(JSON.stringify({ native: 'passed', health: status.health, version: status.version, reference: 'PowerShell current-user AppX + User environment matched', artifacts }, null, 2));
} finally {
  if (browser) await browser.close();
  child.kill();
}
