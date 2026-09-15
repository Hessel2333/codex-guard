// Keep original diagnostic details and codes intact for troubleshooting.
const errors: Record<string, string> = {
  WINDOWS_API_ERROR: 'Windows 接口调用失败', ENVIRONMENT_READ_FAILED: '无法读取环境变量',
  INVALID_INSTALL_LOCATION: '安装位置无效', INVALID_CLI_PATH: 'CLI 路径无效',
  CLI_ACCESS_FAILED: '无法访问 CLI 文件', CLI_NOT_A_FILE: 'CLI 路径不是普通文件', CLI_METADATA_FAILED: '无法读取 CLI 文件信息',
  WORKER_FAILED: '后台任务执行失败', UNSUPPORTED_PLATFORM: '此功能仅支持 Windows',
  CODEX_APP_NOT_FOUND: '未找到 Codex 桌面程序', CONFIRMATION_REQUIRED: '请先确认操作',
  INVALID_APP_PATH: '桌面程序路径无效', INVALID_PROXY: '代理地址无效', INVALID_NO_PROXY: '代理绕过列表无效',
  SETTINGS_READ: '无法读取代理设置', OPERATION_BUSY: '另一项操作正在执行，请稍后重试',
  LAUNCH_FAILED: 'Codex 启动失败', LAUNCH_STATUS: '无法确认启动状态', LAUNCH_TIMEOUT: '等待 Codex 启动超时',
  UAC_CANCELLED: '已取消管理员授权', ELEVATION_FAILED: '管理员授权失败', ELEVATION_PROCESS: '无法启动管理员辅助进程',
  ELEVATION_TIMEOUT: '管理员辅助进程尚未完成', ELEVATION_RESULT: '无法读取管理员操作结果',
  PROCESS_ACCESS_DENIED: '无法访问 Codex 进程', PROCESS_CHANGED: '进程已发生变化，请刷新后重试',
  PROCESS_ENUMERATION: '无法枚举进程', PROCESS_PATH: '无法读取进程路径', PROCESS_SESSION: '无法读取进程会话',
  PROCESS_SNAPSHOT: '无法读取进程列表', PROCESS_TIME: '无法读取进程启动时间',
  STOP_ACCESS_DENIED: '没有权限停止该进程', STOP_FAILED: '停止进程失败', STOP_TIMEOUT: '等待进程结束超时',
  UNVERIFIED_PROCESS: '无法确认进程身份', CLOSE_WINDOWS: '无法关闭进程窗口',
  KNOWN_FOLDER: '无法定位 Windows 用户文件夹', TOKEN_READ: '无法读取进程权限', SELF_PATH: '无法定位启动器自身路径',
  COM_INITIALIZE: '无法初始化 Windows Shell', SHORTCUT_CONFLICT: '快捷方式与其他程序冲突',
  SHORTCUT_CREATE: '无法创建快捷方式', SHORTCUT_EXISTS: '无法检查现有快捷方式', SHORTCUT_ICON: '无法保存快捷方式图标', SHORTCUT_WRITE: '无法保存快捷方式',
  JSON_ENCODE: '无法编码配置', STATE_COMMIT: '无法提交配置', STATE_DIRECTORY: '无法创建配置目录', STATE_LOCK: '无法锁定配置', STATE_READ: '无法读取配置', STATE_WRITE: '无法写入配置',
  LOG_DIRECTORY: '无法创建日志目录', LOG_READ: '无法读取日志', LOG_ROTATE: '无法轮换日志', LOG_WRITE: '无法写入日志',
};
export function errorMessage(code: string, original: string): string { return errors[code] ?? original; }
