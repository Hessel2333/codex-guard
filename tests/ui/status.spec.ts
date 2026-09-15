import { test, expect } from '@playwright/test';
import type { CodexStatus } from '../../src/types';
import release from '../../src/release.json' with { type: 'json' };
test.beforeEach(async ({ page }) => {
  await page.addInitScript(version => { localStorage.setItem('guard.seenRelease', version); localStorage.setItem('guard.autoUpdateCheck', 'false'); }, release.version);
});

const install = 'C:\\Program Files\\WindowsApps\\OpenAI.Codex_26.908.4834.0_x64__2p2nqsd0c76g0';
const expected = `${install}\\app\\resources\\codex.exe`;
const file = { path: expected, exists: true, is_file: true, accessible: true, size: 125000000, modified_unix_ms: 1789340000000, file_version: null, error: null };
const healthy: CodexStatus = {
  installed: true, version: '26.908.4834.0', install_location: install,
  package: { name: 'OpenAI.Codex', version: '26.908.4834.0', install_location: install, package_full_name: 'OpenAI.Codex_26.908.4834.0_x64__2p2nqsd0c76g0', package_family_name: 'OpenAI.Codex_2p2nqsd0c76g0' },
  expected_cli_path: expected, expected_cli_exists: true, current_cli_path: expected, current_cli_exists: true,
  path_matches: true, expected_cli: file, current_cli: file, health: 'healthy', checked_at_unix_ms: 1789340000000, issues: [],
};

// Fixtures live in tests only. Production never substitutes sample status for Windows data.
async function mock(page: import('@playwright/test').Page, value: CodexStatus) {
  await page.addInitScript(status => {
    Object.defineProperty(window, 'isTauri', { value: true });
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async () => status } });
  }, value);
  await page.goto('/');
}

test('healthy overview, diagnostics, refresh and persisted theme', async ({ page }) => {
  await mock(page, healthy);
  await expect(page.locator('html')).toHaveAttribute('lang', 'zh-CN');
  await expect(page.locator('.brand img')).toHaveAttribute('src', '/guard.png');
  await expect(page.getByRole('heading', { name: 'Codex 已就绪' })).toBeVisible();
  await page.getByRole('button', { name: '查看诊断' }).click();
  await expect(page.getByText('125,000,000 字节', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: '运行概览', exact: true }).click();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Codex 已就绪' })).toBeVisible();
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('外观').selectOption('dark');
  await page.keyboard.press('Escape');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('stale path is a warning, never green', async ({ page }) => {
  const current = expected.replace('26.908.4834.0', '26.903.9818.0');
  await mock(page, { ...healthy, health: 'repair_recommended', current_cli_path: current, current_cli_exists: false, path_matches: false, current_cli: { ...file, path: current, exists: false, accessible: false } });
  await expect(page.getByRole('heading', { name: '建议修复启动路径' })).toBeVisible();
  await expect(page.getByText('不一致', { exact: true })).toBeVisible();
  await expect(page.getByText('与当前版本一致。', { exact: true })).toHaveCount(0);
});

test('package not installed renders normal empty state', async ({ page }) => {
  await mock(page, { ...healthy, installed: false, package: null, version: null, install_location: null, expected_cli_path: null, expected_cli_exists: null, path_matches: false, health: 'not_installed' });
  await expect(page.getByRole('heading', { name: '尚未安装 Codex 桌面版' })).toBeVisible();
  await expect(page.getByRole('button', { name: '一键修复路径' })).toHaveCount(0);
});

test('missing bundled CLI renders installation error', async ({ page }) => {
  await mock(page, { ...healthy, expected_cli_exists: false, health: 'installation_incomplete', expected_cli: { ...file, exists: false, accessible: false } });
  await expect(page.getByRole('heading', { name: 'Codex 安装似乎不完整' })).toBeVisible();
  await expect(page.getByRole('button', { name: '一键修复路径' })).toHaveCount(0);
});

test('real backend error is visible and retry recovers without stale green status', async ({ page }) => {
  await page.addInitScript(status => {
    let attempt = 0;
    Object.defineProperty(window, 'isTauri', { value: true });
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async () => {
      if (++attempt === 1) throw { code: 'WINDOWS_API_ERROR', message: 'Read AppX packages', detail: 'Access denied; HRESULT 0x80070005' };
      return status;
    } } });
  }, healthy);
  await page.goto('/');
  await expect(page.getByRole('alert')).toContainText('HRESULT 0x80070005');
  await expect(page.getByRole('heading', { name: 'Codex 已就绪' })).toHaveCount(0);
  await page.getByRole('button', { name: '重试' }).click();
  await expect(page.getByRole('heading', { name: 'Codex 已就绪' })).toBeVisible();
});

test('browser preview explains desktop requirement', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: '请打开 Windows 桌面应用' })).toBeVisible();
});

test('long paths fit native minimum and narrow preview widths', async ({ page }) => {
  await mock(page, healthy);
  for (const width of [1490, 1100, 680, 390]) {
    await page.setViewportSize({ width, height: 780 });
    await expect(page.getByRole('heading', { name: 'Codex 已就绪' })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
});

for (const fail of [false, true]) {
  test(`repair confirmation cancellation and ${fail ? 'write failure' : 'verified success'}`, async ({ page }) => {
    await page.addInitScript(({ healthy, fail }) => {
      let repaired = false;
      (window as any).repairCalls = [];
      Object.defineProperty(window, 'isTauri', { value: true });
      Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: unknown) => {
        if (command === 'get_proxy_startup_intent') return null;
        if (command === 'repair_cli_path') {
          (window as any).repairCalls.push(args);
          if (fail) throw { code: 'ENVIRONMENT_WRITE_FAILED', message: 'Cannot save', detail: 'Access denied' };
          repaired = true;
          return { status: healthy, changed: true, previous_path: null, notification_sent: true };
        }
        return repaired ? healthy : { ...healthy, current_cli_path: null, current_cli_exists: false, path_matches: false, health: 'repair_recommended' };
      } } });
    }, { healthy, fail });
    await page.goto('/');
    await page.getByRole('button', { name: '一键修复路径' }).click();
    await expect(page.getByRole('dialog')).toContainText(expected);
    if (!fail) {
      await page.setViewportSize({ width: 680, height: 780 });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: 'test-results/repair-confirmation.png' });
    }
    await page.getByRole('button', { name: '取消', exact: true }).click();
    expect(await page.evaluate(() => (window as any).repairCalls)).toEqual([]);
    await page.getByRole('button', { name: '一键修复路径' }).click();
    await page.getByRole('button', { name: '确认修复', exact: true }).click();
    expect(await page.evaluate(() => (window as any).repairCalls)).toEqual([{ confirmed: true, expectedPath: expected, previousPath: null }]);
    if (fail) {
      await expect(page.getByRole('alert')).toContainText('无法写入用户环境变量');
      await expect(page.getByRole('heading', { name: 'Codex 已就绪' })).toHaveCount(0);
      await expect(page.getByRole('button', { name: '一键修复路径' })).toBeEnabled();
    } else {
      await expect(page.getByRole('heading', { name: 'Codex 已就绪' })).toBeVisible();
      await expect(page.getByText('启动路径已修复并核验。', { exact: false })).toBeVisible();
      await expect(page.getByRole('button', { name: '一键修复路径' })).toHaveCount(0);
    }
  });
}
