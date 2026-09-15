import { invoke, isTauri } from '@tauri-apps/api/core';
import type { AppError, CodexStatus } from '../types';
import { errorMessage } from './messages';

export async function detect(): Promise<CodexStatus> {
  if (!isTauri()) {
    throw { code: 'DESKTOP_REQUIRED', message: '请打开 Windows 桌面应用',
      detail: '实时检测需要 Tauri 桌面环境，可运行 npm run tauri dev 启动。' } satisfies AppError;
  }
  return invoke<CodexStatus>('get_codex_status');
}

export function appError(error: unknown): AppError {
  if (typeof error === 'object' && error !== null && 'code' in error && 'message' in error && 'detail' in error) {
    return { code: String(error.code), message: errorMessage(String(error.code), String(error.message)), detail: String(error.detail) };
  }
  return { code: 'UNEXPECTED_ERROR', message: '检测未能完成', detail: String(error) };
}
