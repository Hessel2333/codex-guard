import type { Health } from '../types';

export const healthCopy: Record<Health, { title: string; description: string; tone: string }> = {
  healthy: { title: 'Codex 已就绪', description: '当前 CLI 路径与已安装版本一致。', tone: 'success' },
  repair_recommended: { title: '建议修复启动路径', description: 'CODEX_CLI_PATH 需要指向当前 Codex 安装包内的 CLI。', tone: 'warning' },
  not_installed: { title: '尚未安装 Codex 桌面版', description: '当前 Windows 用户下未找到 OpenAI.Codex，请安装 Codex 桌面版后刷新。', tone: 'neutral' },
  installation_incomplete: { title: 'Codex 安装似乎不完整', description: '安装包内的 codex.exe 缺失或不是有效文件，请先检查 Codex 安装。', tone: 'danger' },
  detection_error: { title: '无法确认启动状态', description: 'Windows 无法读取部分必要信息，请在安装诊断中查看详细错误。', tone: 'danger' },
};
export const yesNo = (value: boolean | null) => value === null ? '未知' : value ? '是' : '否';
export const dateTime = (value: number | null) => value === null ? '不可用' : new Date(value).toLocaleString('zh-CN');
export const fileSize = (value: number | null) => value === null ? '不可用' : `${(value / 1024 / 1024).toFixed(2)} MB（${value.toLocaleString('zh-CN')} 字节）`;
