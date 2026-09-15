import type { CodexStatus } from '../types';
import { dateTime, fileSize, yesNo } from '../lib/presentation';
import { errorMessage } from '../lib/messages';

function Rows({ rows }: { rows: [string, string | null][] }) {
  return <dl className="diagnostic-rows">{rows.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value ?? '不可用'}</dd></div>)}</dl>;
}
export function Diagnostics({ status }: { status: CodexStatus }) {
  return <div className="diagnostics">
    <section className="card"><h2>应用包</h2><Rows rows={[
      ['名称', status.package?.name ?? null], ['版本', status.version],
      ['安装位置', status.install_location], ['应用包完整名称', status.package?.package_full_name ?? null],
      ['应用包系列', status.package?.package_family_name ?? null],
    ]} /></section>
    <section className="card"><h2>内置 CLI</h2><Rows rows={[
      ['预期路径', status.expected_cli_path], ['文件存在', yesNo(status.expected_cli_exists)],
      ['普通文件', yesNo(status.expected_cli.is_file)], ['可读取', yesNo(status.expected_cli.accessible)],
      ['大小', fileSize(status.expected_cli.size)], ['修改时间', dateTime(status.expected_cli.modified_unix_ms)],
      ['文件版本', status.expected_cli.file_version ?? '该程序未提供文件版本'],
    ]} /></section>
    <section className="card"><h2>环境变量</h2><Rows rows={[
      ['作用范围', '当前 Windows 用户（HKCU）'], ['CODEX_CLI_PATH', status.current_cli_path],
      ['路径存在', yesNo(status.current_cli_exists)], ['可读取', yesNo(status.current_cli.accessible)],
      ['匹配当前版本', yesNo(status.path_matches)],
    ]} /></section>
    {status.issues.length > 0 && <section className="card"><h2>检测问题</h2>{status.issues.map((issue, index) => <div className="issue" key={`${issue.code}-${index}`}><strong>{errorMessage(issue.code, issue.message)}</strong><p>{issue.code}</p><code>{issue.detail}</code></div>)}</section>}
  </div>;
}
