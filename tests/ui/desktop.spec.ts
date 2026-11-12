import { test, expect, type Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import release from '../../src/release.json' with { type: 'json' };

type Scenario = 'normal' | 'conflict' | 'external' | 'invalid' | 'no_target';
async function prepare(page: Page, scenario: Scenario = 'normal') {
  await page.addInitScript(({ scenario, version }) => {
    localStorage.setItem('guard.seenRelease', version);
    localStorage.setItem('guard.autoUpdateCheck', 'false');
    const calls: { name: string; args?: any }[] = [];
    (window as any).__desktopCalls = calls;
    const file = { path: null, exists: null, is_file: null, accessible: null, size: null, modified_unix_ms: null, file_version: null, error: null };
    const status: any = {
      home: 'C:\\Users\\Fixture\\.codex', home_source: 'default', home_runtime_verified: false,
      config_path: 'C:\\Users\\Fixture\\.codex\\config.toml', config_exists: true, revision: 'revision-1',
      settings: [
        { key: 'developer_instructions', value: '原有工作偏好', advanced: false },
        { key: 'model_reasoning_effort', value: 'medium', advanced: false },
        { key: 'approval_policy', value: 'on-request', advanced: false },
        { key: 'sandbox_mode', value: 'workspace-write', advanced: false },
      ],
      config_error: scenario === 'invalid' ? { code: 'DESKTOP_TOML', message: '配置文件格式有误', detail: '原文件保持不变。' } : null,
      external_manager: scenario === 'external',
      target: scenario === 'no_target' ? null : { executable: 'C:\\Program Files\\Codex\\Codex.exe', cli_path: 'C:\\Program Files\\Codex\\resources\\codex.exe', identity: 'target-1', source: 'discovered', running_pids: [42], unverified_processes: 0 },
      target_error: scenario === 'no_target' ? { code: 'DESKTOP_NOT_FOUND', message: '未找到 Codex 桌面应用', detail: '请核对安装路径。' } : null,
      history: [], history_error: null, backup_path: 'C:\\Users\\Fixture\\AppData\\Local\\CodexBootGuard\\desktop-config\\fixture', checked_at: Date.now(),
    };
    (window as any).__desktopState = status;
    const evidence = () => ({ home: status.home, revision: status.revision, target_identity: 'target-1', evidence: { cli_version: 'codex-cli fixture', checked_at: Date.now(), verified_keys: ['developer_instructions'], scope: 'isolated_user_settings', active_session_verified: false } });
    Object.defineProperty(window, 'isTauri', { value: true });
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (name: string, args?: any) => {
      calls.push({ name, args });
      if (name === 'get_proxy_startup_intent') return null;
      if (name === 'get_codex_status') return { installed: false, version: null, install_location: null, package: null, expected_cli_path: null, current_cli_path: null, expected_cli_exists: null, current_cli_exists: null, path_matches: false, expected_cli: file, current_cli: file, health: 'not_installed', issues: [], checked_at_unix_ms: Date.now() };
      if (name === 'get_desktop_config_status') return structuredClone({ ...status, ...(args.selection.home ? { home: args.selection.home, home_source: 'selected', config_path: `${args.selection.home}\\config.toml` } : {}) });
      if (name === 'verify_desktop_config') return evidence();
      if (name === 'launch_desktop_config') return { pid: 88, started_unix_ms: Date.now(), executable: status.target.executable, administrator: false, launched_at_unix_ms: Date.now(), codex_home: args.request.selection.home, proxy_enabled: args.request.use_proxy };
      if (name === 'preview_desktop_config') return { home: status.home, revision: status.revision, target_identity: 'target-1', after_revision: 'revision-2', changes: args.edits.map((e: any) => ({ key: e.key, before: status.settings.find((s: any) => s.key === e.key).value, after: e.value })), operation: 'apply', snapshot_id: null };
      if (name === 'preview_desktop_restore') return { home: status.home, revision: status.revision, target_identity: 'target-1', after_revision: 'revision-3', changes: [{ key: 'developer_instructions', before: status.settings[0].value, after: '原有工作偏好' }], operation: 'restore', snapshot_id: args.snapshotId };
      if (name === 'apply_desktop_config') {
        if (scenario === 'conflict') throw { code: 'DESKTOP_STALE', message: '配置已发生变化，请重新预览', detail: '未覆盖其他操作的修改。' };
        await new Promise(resolve => setTimeout(resolve, 180));
        for (const edit of args.request.edits) status.settings.find((s: any) => s.key === edit.key).value = edit.value;
        status.revision = 'revision-2';
        status.history = [{ id: '123-1-1', created_at: Date.now(), operation: 'apply', state: 'committed', can_restore: true, note: '可完整恢复本次修改前的文件' }];
        return { receipt: { changed: true, snapshot_id: '123-1-1', revision: status.revision }, verification: evidence() };
      }
      if (name === 'restore_desktop_config') {
        status.settings[0].value = '原有工作偏好'; status.revision = 'revision-3';
        status.history[0].can_restore = false;
        return { receipt: { changed: true, snapshot_id: '124-1-1', revision: status.revision }, verification: evidence() };
      }
      throw Error(`Unexpected command: ${name}`);
    } } });
  }, { scenario, version: release.version });
  await page.goto('/');
  await page.getByRole('button', { name: '桌面配置', exact: true }).click();
  await expect(page.getByRole('heading', { name: '桌面实例', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
}
const calls = (page: Page, name: string) => page.evaluate(name => (window as any).__desktopCalls.filter((c: any) => c.name === name), name);

for (const proxy of [true, false]) {
  test(`configuration launch keeps the selected home with proxy=${proxy}`, async ({ page }) => {
    await prepare(page);
    const checkbox = page.getByRole('checkbox', { name: '使用已保存的代理', exact: true });
    await expect(checkbox).toBeChecked();
    if (!proxy) await checkbox.uncheck();
    await page.getByRole('button', { name: '启动此配置', exact: true }).click();
    await expect(page.getByRole('dialog')).toContainText('C:\\Users\\Fixture\\.codex');
    await expect(page.getByRole('dialog')).toContainText(proxy ? '同时使用已保存的代理' : '不注入 Guard 代理');
    expect(await calls(page, 'launch_desktop_config')).toHaveLength(0);
    await page.getByRole('button', { name: '确认启动', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Codex 已使用此配置启动' })).toBeVisible();
    expect((await calls(page, 'launch_desktop_config'))[0].args.request).toEqual({ selection: { home: 'C:\\Users\\Fixture\\.codex' }, revision: 'revision-1', target_identity: 'target-1', use_proxy: proxy, confirmed: true });
  });
}
test('cancelled configuration launch does not restart Codex', async ({ page }) => {
  await prepare(page);
  await page.getByRole('button', { name: '启动此配置', exact: true }).click();
  await page.getByRole('button', { name: '取消', exact: true }).click();
  expect(await calls(page, 'launch_desktop_config')).toHaveLength(0);
});
test('external managed instructions can be launched with a proxy without taking ownership', async ({ page }) => {
  await prepare(page, 'external');
  await expect(page.getByLabel('自定义指令', { exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '启动此配置', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: '启动此配置', exact: true }).click();
  await page.getByRole('button', { name: '确认启动', exact: true }).click();
  expect((await calls(page, 'launch_desktop_config'))[0].args.request.use_proxy).toBe(true);
  expect(await calls(page, 'apply_desktop_config')).toHaveLength(0);
});
test('unsaved configuration edits disable configuration launch', async ({ page }) => {
  await prepare(page);
  await page.getByLabel('自定义指令', { exact: true }).fill('尚未保存');
  await expect(page.getByRole('button', { name: '启动此配置', exact: true })).toBeDisabled();
});

test('instance and configuration evidence never claim a live session is verified', async ({ page }) => {
  await prepare(page);
  await expect(page.getByText('可读取，格式有效')).toBeVisible();
  await expect(page.getByText('尚未验证；不能据此判断云端任务已生效')).toBeVisible();
  await page.getByRole('button', { name: '检查兼容性', exact: true }).click();
  await expect(page.getByText('常规配置项已通过隔离检查')).toBeVisible();
  expect(await calls(page, 'verify_desktop_config')).toEqual([{ name: 'verify_desktop_config', args: { selection: { home: null }, revision: 'revision-1', targetIdentity: 'target-1' } }]);
  await expect(page.getByText('尚未验证；不能据此判断云端任务已生效')).toBeVisible();
});
test('cancelled preview never writes and preserves edits', async ({ page }) => {
  await prepare(page);
  await page.getByLabel('自定义指令', { exact: true }).fill('用中文解释测试结果');
  await page.getByRole('button', { name: '预览变更', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('原有工作偏好');
  await expect(page.getByRole('dialog')).toContainText('用中文解释测试结果');
  await page.keyboard.press('Escape');
  expect(await calls(page, 'apply_desktop_config')).toHaveLength(0);
  await expect(page.getByLabel('自定义指令', { exact: true })).toHaveValue('用中文解释测试结果');
});
test('confirmed apply sends only selected keys and refreshes backup history', async ({ page }) => {
  await prepare(page);
  await page.getByLabel('自定义指令', { exact: true }).fill('用中文解释测试结果');
  await page.getByLabel('推理强度', { exact: true }).selectOption('low');
  await page.getByRole('button', { name: '预览变更', exact: true }).click();
  await page.getByRole('button', { name: '验证并应用', exact: true }).click();
  await expect(page.getByRole('button', { name: '运行概览', exact: true })).toBeDisabled();
  await expect(page.getByRole('status').filter({ hasText: '配置已保存，并已通过兼容性检查' })).toBeVisible();
  const requests = await calls(page, 'apply_desktop_config');
  expect(requests).toHaveLength(1);
  expect(requests[0].args.request).toEqual({ selection: { home: null }, revision: 'revision-1', target_identity: 'target-1', edits: [{ key: 'developer_instructions', value: '用中文解释测试结果' }, { key: 'model_reasoning_effort', value: 'low' }], confirmed: true });
  await expect(page.getByRole('button', { name: '预览变更', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '预览恢复', exact: true })).toBeEnabled();
});
test('restore requires preview and a separate confirmation', async ({ page }) => {
  await prepare(page);
  await page.getByLabel('自定义指令', { exact: true }).fill('新的工作偏好');
  await page.getByRole('button', { name: '预览变更', exact: true }).click();
  await page.getByRole('button', { name: '验证并应用', exact: true }).click();
  await expect(page.getByRole('button', { name: '预览恢复', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: '预览恢复', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('确认恢复配置');
  expect(await calls(page, 'restore_desktop_config')).toHaveLength(0);
  await page.getByRole('button', { name: '验证并恢复', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: '配置已恢复' })).toBeVisible();
  await expect(page.getByLabel('自定义指令', { exact: true })).toHaveValue('原有工作偏好');
  expect((await calls(page, 'restore_desktop_config'))[0].args.request.snapshot_id).toBe('123-1-1');
});
test('failed apply retains draft without displaying success', async ({ page }) => {
  await prepare(page, 'conflict');
  await page.getByLabel('自定义指令', { exact: true }).fill('保留我的编辑');
  await page.getByRole('button', { name: '预览变更', exact: true }).click();
  await page.getByRole('button', { name: '验证并应用', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('配置已发生变化');
  await expect(page.getByLabel('自定义指令', { exact: true })).toHaveValue('保留我的编辑');
  await expect(page.getByText('配置已保存，并已通过兼容性检查', { exact: false })).toHaveCount(0);
});
for (const scenario of ['external', 'invalid', 'no_target'] as Scenario[]) {
  test(`unsafe or unavailable configuration is read-only: ${scenario}`, async ({ page }) => {
    await prepare(page, scenario);
    await expect(page.getByLabel('自定义指令', { exact: true })).toBeDisabled();
    await expect(page.getByRole('button', { name: '预览变更', exact: true })).toBeDisabled();
    await expect(page.getByRole('alert')).toBeVisible();
  });
}
test('refresh retains drafts and invalidates stale compatibility evidence', async ({ page }) => {
  await prepare(page);
  await page.getByRole('button', { name: '检查兼容性', exact: true }).click();
  await expect(page.getByText('常规配置项已通过隔离检查')).toBeVisible();
  await page.getByLabel('自定义指令', { exact: true }).fill('未保存的编辑');
  await page.evaluate(() => { (window as any).__desktopState.revision = 'external-revision'; });
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  await expect(page.getByText('常规配置项已通过隔离检查')).toHaveCount(0);
  await expect(page.getByLabel('自定义指令', { exact: true })).toHaveValue('未保存的编辑');
});
test('directory switch and page navigation guard unsaved edits', async ({ page }) => {
  await prepare(page);
  await page.getByLabel('自定义指令', { exact: true }).fill('不要丢失');
  await page.getByRole('button', { name: '运行概览', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('离开桌面配置');
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await page.getByText('选择其他配置目录', { exact: true }).click();
  await page.getByLabel('Codex 配置目录', { exact: true }).fill('D:\\CodexHome');
  await page.getByRole('button', { name: '读取此目录', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('切换配置目录');
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect(page.getByLabel('自定义指令', { exact: true })).toHaveValue('不要丢失');
});
test('removing a setting is explicit and does not change other permission options', async ({ page }) => {
  await prepare(page);
  await page.getByRole('button', { name: '移除自定义指令', exact: true }).click();
  await page.getByRole('button', { name: '预览变更', exact: true }).click();
  const request = (await calls(page, 'preview_desktop_config'))[0];
  expect(request.args.edits).toEqual([{ key: 'developer_instructions', value: null }]);
});
test('desktop and mobile layouts are usable, theme-aware, and console-clean', async ({ page }, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await prepare(page);
  expect(await page.title()).toContain('Codex Guard');
  await expect(page.locator('vite-error-overlay')).toHaveCount(0);
  const out = path.join(os.tmpdir(), `codex-guard-desktop-qa-${process.pid}`);
  await mkdir(out, { recursive: true });
  await page.screenshot({ path: path.join(out, 'desktop-config-light.png'), fullPage: true });
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByLabel('外观').selectOption('dark');
  await page.keyboard.press('Escape');
  await page.screenshot({ path: path.join(out, 'desktop-config-dark.png'), fullPage: true });
  for (const width of [680, 390]) {
    await page.setViewportSize({ width, height: 844 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await expect(page.getByRole('button', { name: '检查兼容性', exact: true })).toBeVisible();
  }
  await page.screenshot({ path: path.join(out, 'desktop-config-mobile.png'), fullPage: true });
  expect(errors).toEqual([]);
  await testInfo.attach('Desktop config screenshot location', { body: out, contentType: 'text/plain' });
});
