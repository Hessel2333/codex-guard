import type { CodexStatus } from '../types';
import { healthCopy } from '../lib/presentation';
import { Icon } from './Icon';

export function Overview({ status, showDiagnostics, onRepair, busy }: { status: CodexStatus; showDiagnostics: () => void; onRepair: () => void; busy: boolean }) {
  const copy = healthCopy[status.health];
  const cliCorrect = status.path_matches && status.current_cli_exists === true && status.current_cli.accessible === true;
  return <>
    <section className="card health-card" aria-labelledby="health-title">
      <p className="section-label">Codex 启动状态</p>
      <h2 id="health-title"><span className={`status-dot ${copy.tone}`} />{copy.title}</h2>
      <p className="health-description">{copy.description}</p>
      <dl className="facts">
        <div><dt>Codex 桌面版</dt><dd>{status.version ?? '未检测到'}</dd></div>
        <div><dt>CLI 路径</dt><dd>{!status.installed ? '不可用' : cliCorrect ? '正确' : '需要检查'}</dd></div>
        <div><dt>环境变量</dt><dd>{status.issues.some(i => i.code === 'ENVIRONMENT_READ_FAILED') ? '未知' : !status.current_cli_path ? '未设置' : status.path_matches ? '已同步' : '不一致'}</dd></div>
      </dl>
      <div className="card-actions">{status.health === 'repair_recommended' && <button className="primary-button" disabled={busy} onClick={onRepair}>一键修复路径</button>}<button onClick={showDiagnostics}>查看诊断 <Icon name="chevron" /></button></div>
    </section>
    <section className="card path-card" aria-labelledby="path-title">
      <h2 id="path-title">CLI 路径</h2>
      <dl className="path-list">
        <div><dt>当前路径</dt><dd><code>{status.current_cli_path || '未设置'}</code></dd></div>
        <div><dt>预期路径</dt><dd><code>{status.expected_cli_path ?? '不可用：尚未安装 Codex 桌面版'}</code></dd></div>
      </dl>
      <p className={`path-verdict ${status.path_matches ? 'success' : 'muted'}`}>
        {status.path_matches && <span className="check-circle"><Icon name="check" /></span>}
        {status.path_matches ? '与当前版本一致。' : !status.installed ? '安装 Codex 桌面版后可确定预期 CLI 路径。' : '当前路径与已安装版本不一致。'}
      </p>
    </section>
  </>;
}
