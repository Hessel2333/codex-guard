import { invoke, isTauri } from '@tauri-apps/api/core';
import type { AppError, LaunchReceipt } from '../types';

export const desktopKeys = ['developer_instructions', 'model_reasoning_effort', 'approval_policy', 'sandbox_mode'] as const;
export type DesktopKey = typeof desktopKeys[number];
export interface DesktopSelection { home: string | null }
export interface DesktopEdit { key: DesktopKey; value: string | null }
export interface DesktopTarget {
  executable: string; cli_path: string; desktop_version: string | null; identity: string; source: 'manual' | 'discovered';
  running_pids: number[]; unverified_processes: number;
}
export interface DesktopSetting { key: DesktopKey; value: string | null; advanced: boolean }
export interface DesktopHistory {
  id: string; created_at: number; operation: string; state: string; can_restore: boolean; note: string;
}
export interface DesktopStatus {
  home: string; home_source: 'selected' | 'environment' | 'default'; home_runtime_verified: boolean;
  config_path: string; config_exists: boolean; revision: string | null; settings: DesktopSetting[];
  config_error: AppError | null; external_manager: boolean; target: DesktopTarget | null;
  target_error: AppError | null; history: DesktopHistory[]; history_error: AppError | null;
  backup_path: string; checked_at: number;
}
export interface DesktopChange { key: DesktopKey; before: string | null; after: string | null }
export interface DesktopPreview {
  home: string; revision: string; target_identity: string; after_revision: string;
  changes: DesktopChange[]; operation: 'apply' | 'restore'; snapshot_id: string | null;
}
export interface DesktopProbe {
  home: string; revision: string; target_identity: string;
  evidence: { cli_version: string; checked_at: number; verified_keys: string[]; scope: string; active_session_verified: boolean };
}
export interface DesktopResult {
  receipt: { changed: boolean; snapshot_id: string | null; revision: string };
  verification: DesktopProbe;
}
function call<T>(name: string, args: Record<string, unknown>): Promise<T> {
  if (!isTauri()) return Promise.reject({ code: 'DESKTOP_REQUIRED', message: '请打开 Codex Guard 桌面应用', detail: '此页面需要访问本机 Codex 安装和配置。' } satisfies AppError);
  return invoke<T>(name, args);
}
export const desktopStatus = (selection: DesktopSelection) => call<DesktopStatus>('get_desktop_config_status', { selection });
export interface DesktopLaunchRequest { selection: DesktopSelection; revision: string; target_identity: string; use_proxy: boolean; confirmed: boolean }
export const launchDesktop = (request: DesktopLaunchRequest) => call<LaunchReceipt>('launch_desktop_config', { request });
export const previewDesktop = (selection: DesktopSelection, status: DesktopStatus, edits: DesktopEdit[]) => call<DesktopPreview>('preview_desktop_config', { selection, revision: status.revision, targetIdentity: status.target?.identity, edits });
export const previewDesktopRestore = (selection: DesktopSelection, status: DesktopStatus, snapshotId: string) => call<DesktopPreview>('preview_desktop_restore', { selection, revision: status.revision, targetIdentity: status.target?.identity, snapshotId });
export const verifyDesktop = (selection: DesktopSelection, status: DesktopStatus) => call<DesktopProbe>('verify_desktop_config', { selection, revision: status.revision, targetIdentity: status.target?.identity });
export const applyDesktop = (selection: DesktopSelection, preview: DesktopPreview, edits: DesktopEdit[]) => call<DesktopResult>('apply_desktop_config', { request: { selection, revision: preview.revision, target_identity: preview.target_identity, edits, confirmed: true } });
export const restoreDesktop = (selection: DesktopSelection, preview: DesktopPreview) => call<DesktopResult>('restore_desktop_config', { request: { selection, revision: preview.revision, target_identity: preview.target_identity, snapshot_id: preview.snapshot_id, confirmed: true } });
