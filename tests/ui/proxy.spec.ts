import { test, expect, type Page } from '@playwright/test';
import release from '../../src/release.json' with { type: 'json' };
test.beforeEach(async ({ page }) => {
  await page.addInitScript(version => { localStorage.setItem('guard.seenRelease', version); localStorage.setItem('guard.autoUpdateCheck', 'false'); }, release.version);
});

async function prepare(page: Page, intent: string | null = null) {
  await page.addInitScript(startupIntent => {
    const settings = { http_proxy: 'http://127.0.0.1:7890', all_proxy: 'socks5://127.0.0.1:7890', no_proxy: 'localhost,127.0.0.1,::1', app_path: null };
    const file = { path: null, exists: null, is_file: null, accessible: null, size: null, modified_unix_ms: null, file_version: null, error: null };
    const process = { pid: 42, parent_pid: 10, name: 'Codex.exe', path: 'C:\\Codex\\Codex.exe', started_unix_ms: 1000, managed: true, error: null };
    let processes = [process];
    const calls: { name: string; args: unknown }[] = [];
    Object.defineProperty(window, '__proxyCalls', { value: calls });
    Object.defineProperty(window, 'isTauri', { value: true });
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (name: string, args?: Record<string, unknown>) => {
      calls.push({ name, args });
      if (name === 'get_proxy_startup_intent') return startupIntent;
      if (name === 'get_codex_status') return { installed: false, version: null, install_location: null, package: null, expected_cli_path: null, current_cli_path: null, expected_cli_exists: null, current_cli_exists: null, path_matches: false, expected_cli: file, current_cli: file, health: 'not_installed', issues: [], checked_at_unix_ms: Date.now() };
      if (name === 'get_proxy_status') return { settings: { ...settings }, settings_path: 'C:\\User\\CodexBootGuard\\proxy-settings.json', executable: 'C:\\Codex\\Codex.exe', discovery_error: null, processes: [...processes], administrator: false, last_launch: null, checked_at_unix_ms: Date.now() };
      if (name === 'get_proxy_logs') return { current: 'New launcher log', legacy: 'Original PowerShell launcher log', current_path: 'C:\\User\\CodexBootGuard\\proxy.log', legacy_path: 'C:\\User\\codex-proxy\\codex-proxy.log' };
      if (name === 'save_proxy_settings') {
        const next = args?.settings as typeof settings;
        if (next.http_proxy === 'invalid') throw { code: 'INVALID_PROXY', message: '代理地址无效', detail: 'Enter a full URL.' };
        Object.assign(settings, next); return { ...settings };
      }
      if (name === 'launch_codex_proxy') {
        if (args?.administrator) throw { code: 'UAC_CANCELLED', message: '管理员启动 was not completed', detail: 'The operation was canceled by the user.' };
        return { pid: 99, started_unix_ms: 3000, executable: 'C:\\Codex\\Codex.exe', administrator: false, launched_at_unix_ms: Date.now() };
      }
      if (name === 'stop_codex_proxy') { processes = []; return [42]; }
      if (name === 'install_proxy_shortcuts') return ['C:\\Desktop\\Codex Guard - 启动代理.lnk', 'C:\\Desktop\\Codex Guard - 管理员代理.lnk'];
      throw Error(`Unexpected command ${name}`);
    } } });
  }, intent);
  await page.goto('/');
  if (!intent) await page.getByRole('button', { name: '代理启动器', exact: true }).click();
  await expect(page.getByRole('heading', { name: '代理启动器', exact: true })).toBeVisible();
}

async function calls(page: Page, name: string) {
  return page.evaluate(command => (window as unknown as { __proxyCalls: { name: string; args: unknown }[] }).__proxyCalls.filter(call => call.name === command), name);
}

test('proxy fields save and dirty settings disable launch', async ({ page }) => {
  await prepare(page);
  await page.getByLabel('HTTP / HTTPS 代理', { exact: false }).fill('http://127.0.0.1:7897');
  await expect(page.getByRole('button', { name: '使用代理启动', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: '保存设置', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: '代理设置已保存' })).toBeVisible();
  await expect(page.getByRole('button', { name: '使用代理启动', exact: true })).toBeEnabled();
  expect(await calls(page, 'save_proxy_settings')).toHaveLength(1);
});

test('canceling restart and stop does not invoke destructive commands', async ({ page }) => {
  await prepare(page);
  for (const name of ['重启', '停止 Codex', '使用代理启动', '管理员启动']) {
    await page.getByRole('button', { name, exact: true }).click();
    await expect(page.getByRole('dialog')).toContainText('未保存的工作和正在执行的任务');
    await page.getByRole('button', { name: '取消', exact: true }).click();
  }
  expect(await calls(page, 'stop_codex_proxy')).toHaveLength(0);
  expect(await calls(page, 'launch_codex_proxy')).toHaveLength(0);
});

test('confirmed launch and stop invoke structured backend commands', async ({ page }) => {
  await prepare(page);
  await page.getByRole('button', { name: '使用代理启动', exact: true }).click();
  await page.getByRole('button', { name: '确认启动' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Codex 已使用代理启动' })).toBeVisible();
  expect(await calls(page, 'launch_codex_proxy')).toEqual([{ name: 'launch_codex_proxy', args: { administrator: false, confirmed: true } }]);
  await page.getByRole('button', { name: '停止 Codex', exact: true }).click();
  await page.getByRole('button', { name: '停止已确认的进程' }).click();
  await expect(page.getByRole('status').filter({ hasText: '已停止 1 个已确认' })).toBeVisible();
});

test('UAC cancellation is shown as an error, never success', async ({ page }) => {
  await prepare(page);
  await page.getByRole('button', { name: '管理员启动', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('Windows 将通过 UAC 请求管理员授权');
  await page.getByRole('button', { name: '确认启动' }).click();
  await expect(page.getByRole('alert')).toContainText('UAC_CANCELLED');
  await expect(page.getByText('Codex 已使用代理启动', { exact: false })).toHaveCount(0);
});

test('invalid settings retain edits and display backend reason', async ({ page }) => {
  await prepare(page);
  await page.getByLabel('HTTP / HTTPS 代理', { exact: false }).fill('invalid');
  await page.getByRole('button', { name: '保存设置', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('INVALID_PROXY');
  await expect(page.getByLabel('HTTP / HTTPS 代理', { exact: false })).toHaveValue('invalid');
});

test('original logs, shortcuts and narrow layout', async ({ page }) => {
  await prepare(page);
  await page.getByRole('button', { name: '原启动器', exact: true }).click();
  await expect(page.locator('.log-view')).toContainText('Original PowerShell launcher log');
  await page.getByRole('button', { name: '创建或更新快捷方式' }).click();
  await expect(page.getByRole('status').filter({ hasText: '已创建快捷方式：' })).toBeVisible();
  await page.setViewportSize({ width: 680, height: 780 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('shortcut intent opens confirmation without launching anything', async ({ page }) => {
  await prepare(page, 'admin');
  await expect(page.getByRole('dialog')).toContainText('以管理员身份启动?');
  expect(await calls(page, 'launch_codex_proxy')).toHaveLength(0);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
});
