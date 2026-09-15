import { invoke, isTauri } from '@tauri-apps/api/core';
import type { AppError, ProxySettings, ProxyStatus, ProxyLogs, LaunchReceipt } from '../types';

function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) return Promise.reject({ code: 'DESKTOP_REQUIRED', message: '请打开 Windows 桌面应用', detail: '代理管理需要使用桌面 EXE，或运行 npm run tauri dev。' } satisfies AppError);
  return invoke<T>(name, args);
}
export const getProxyStatus = () => call<ProxyStatus>('get_proxy_status');
export const saveProxySettings = (settings: ProxySettings) => call<ProxySettings>('save_proxy_settings', { settings });
export const launchProxy = (administrator: boolean) => call<LaunchReceipt>('launch_codex_proxy', { administrator, confirmed: true });
export const stopProxy = () => call<number[]>('stop_codex_proxy', { confirmed: true });
export const getProxyLogs = () => call<ProxyLogs>('get_proxy_logs');
export const installProxyShortcuts = () => call<string[]>('install_proxy_shortcuts');
export const proxyStartupIntent = () => call<string | null>('get_proxy_startup_intent');
