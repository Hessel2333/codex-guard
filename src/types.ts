export interface AppError { code: string; message: string; detail: string }
export interface AppPackage {
  name: string; version: string; install_location: string;
  package_full_name: string; package_family_name: string;
}
export interface FileStatus {
  path: string | null; exists: boolean | null; is_file: boolean | null;
  accessible: boolean | null; size: number | null; modified_unix_ms: number | null;
  file_version: string | null; error: AppError | null;
}
export type Health = 'healthy' | 'repair_recommended' | 'not_installed' | 'installation_incomplete' | 'detection_error';
export interface CodexStatus {
  installed: boolean; version: string | null; install_location: string | null;
  package: AppPackage | null; expected_cli_path: string | null;
  expected_cli_exists: boolean | null; current_cli_path: string | null;
  current_cli_exists: boolean | null; path_matches: boolean;
  expected_cli: FileStatus; current_cli: FileStatus; health: Health;
  checked_at_unix_ms: number; issues: AppError[];
}

export interface ProxySettings {
  http_proxy: string; all_proxy: string; no_proxy: string; app_path: string | null;
}

export interface RepairResult { status: CodexStatus; changed: boolean; previous_path: string | null; notification_sent: boolean }
export interface ProxyProcess {
  pid: number; parent_pid: number; name: string; path: string | null;
  started_unix_ms: number | null; managed: boolean; error: string | null;
}
export interface LaunchReceipt {
  pid: number; started_unix_ms: number; executable: string; administrator: boolean; launched_at_unix_ms: number;
}
export interface ProxyStatus {
  settings: ProxySettings; settings_path: string; executable: string | null;
  discovery_error: AppError | null; processes: ProxyProcess[]; administrator: boolean;
  last_launch: LaunchReceipt | null; checked_at_unix_ms: number;
}
export interface ProxyLogs { current: string; legacy: string; current_path: string; legacy_path: string }
