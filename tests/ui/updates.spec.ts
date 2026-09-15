import { test, expect, type Page } from '@playwright/test';
import release from '../../src/release.json' with { type: 'json' };

async function prepare(page: Page, mode: 'latest' | 'available' | 'failure' = 'latest', seen = true, automatic = false) {
  await page.addInitScript(({ version, mode, seen, automatic }) => {
    if (!sessionStorage.getItem('updateTestInitialized')) {
      if (seen) localStorage.setItem('guard.seenRelease', version);
      localStorage.setItem('guard.autoUpdateCheck', String(automatic));
      sessionStorage.setItem('updateTestInitialized', 'true');
    }
    (window as any).updateCalls = [];
    Object.defineProperty(window, 'isTauri', { value: true });
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {
      transformCallback: () => 1,
      invoke: async (command: string, args: any) => {
        (window as any).updateCalls.push(command);
        if (command === 'plugin:updater|check') {
          if (mode === 'failure') throw 'network unavailable';
          return mode === 'latest' ? null : { rid: 42, currentVersion: version, version: '0.5.0', body: '新版功能说明\n修复问题', rawJson: {} };
        }
        if (command === 'plugin:updater|download_and_install') {
          args.onEvent.onmessage({ event: 'Started', data: { contentLength: 100 } });
          args.onEvent.onmessage({ event: 'Progress', data: { chunkLength: 50 } });
          await new Promise(r => setTimeout(r, 200));
          throw 'signature verification failed';
        }
        if (command === 'get_proxy_startup_intent') return null;
        if (command === 'get_codex_status') throw { code: 'DESKTOP_REQUIRED', message: '测试检测占位', detail: '' };
        return null;
      }
    } });
  }, { version: release.version, mode, seen, automatic });
  await page.goto('/');
}

test('installed release notes appear once and can be reopened offline', async ({ page }) => {
  await prepare(page, 'failure', false);
  await expect(page.getByRole('dialog')).toContainText(`本次更新内容 · ${release.version}`);
  await expect(page.getByRole('dialog')).toContainText(release.notes.split('\n')[0]);
  await page.getByRole('button', { name: '知道了' }).click();
  await page.reload();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByRole('button', { name: '查看本次更新内容' }).click();
  await expect(page.getByRole('dialog')).toContainText(release.notes);
});

test('manual check reports latest and automatic preference persists', async ({ page }) => {
  await prepare(page);
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await expect(page.getByLabel('自动检查更新')).not.toBeChecked();
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await expect(page.getByText('当前已是最新版本。')).toBeVisible();
  await page.getByLabel('自动检查更新').check();
  await page.reload();
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await expect(page.getByLabel('自动检查更新')).toBeChecked();
});

test('automatic check finds update but never starts installing', async ({ page }) => {
  await prepare(page, 'available', true, true);
  await expect(page.getByText('Codex Guard 0.5.0 已可更新')).toBeVisible({ timeout: 7000 });
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: string) => c.includes('download')))).toEqual([]);
  await page.getByRole('button', { name: '查看更新', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('新版功能说明');
  await page.getByRole('button', { name: '下载并安装更新' }).click();
  await expect(page.getByRole('progressbar')).toBeVisible();
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('signature verification failed');
  await expect(page.getByRole('button', { name: '下载并安装更新' })).toBeEnabled();
  await page.screenshot({ path: 'test-results/update-settings.png' });
});

test('failed manual check reports error and stays retryable', async ({ page }) => {
  await prepare(page, 'failure');
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('network unavailable');
  await expect(page.getByRole('button', { name: '检查更新', exact: true })).toBeEnabled();
});

test('automatic checks recur every six hours and stop when disabled', async ({ page }) => {
  await page.clock.install();
  await prepare(page, 'latest', true, true);
  await page.clock.fastForward(4000);
  await expect.poll(() => page.evaluate(() => (window as any).updateCalls.filter((c: string) => c === 'plugin:updater|check').length)).toBe(1);
  await page.clock.fastForward(6 * 60 * 60 * 1000);
  await expect.poll(() => page.evaluate(() => (window as any).updateCalls.filter((c: string) => c === 'plugin:updater|check').length)).toBe(2);
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('自动检查更新').uncheck();
  await page.clock.fastForward(6 * 60 * 60 * 1000);
  expect(await page.evaluate(() => (window as any).updateCalls.filter((c: string) => c === 'plugin:updater|check').length)).toBe(2);
});
