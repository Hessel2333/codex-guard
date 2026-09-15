import { useCallback, useEffect, useRef, useState } from 'react';
import type { AppError, CodexStatus } from './types';
import { appError, detect, repairPath } from './lib/api';
import { ConfirmDialog } from './components/ConfirmDialog';
import { Icon } from './components/Icon';
import { Overview } from './components/Overview';
import { Diagnostics } from './components/Diagnostics';
import { ProxyLauncher } from './components/ProxyLauncher';
import { proxyStartupIntent } from './lib/proxy-api';
import { Settings, type Theme } from './components/Settings';
import './App.css';

function initialTheme(): Theme {
  try { const value = localStorage.getItem('theme'); return value === 'light' || value === 'dark' ? value : 'system'; }
  catch { return 'system'; }
}

export default function App() {
  const [status, setStatus] = useState<CodexStatus | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [repairTarget, setRepairTarget] = useState<CodexStatus | null>(null);
  const [repairing, setRepairing] = useState(false);
  const [repairError, setRepairError] = useState<AppError | null>(null);
  const [repairNotice, setRepairNotice] = useState('');
  const [tab, setTab] = useState<'overview' | 'diagnostics' | 'proxy'>('overview');
  const [proxyRefresh, setProxyRefresh] = useState(0);
  const [startupIntent, setStartupIntent] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const pending = useRef(false);
  const refresh = useCallback(async () => {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setError(null);
    setRepairError(null);
    setRepairNotice('');
    try { setStatus(await detect()); }
    catch (e) { setError(appError(e)); setStatus(null); }
    finally { pending.current = false; setBusy(false); }
  }, []);
  const runRepair = async () => {
    if (pending.current || !repairTarget) return;
    const target = repairTarget;
    setRepairTarget(null);
    pending.current = true; setBusy(true); setRepairing(true); setRepairError(null); setRepairNotice('');
    try {
      const result = await repairPath(target);
      setStatus(result.status);
      setRepairNotice(result.changed
        ? `启动路径已修复并核验。请重新启动 Codex；已运行的终端需重新打开，必要时注销后登录。${result.notification_sent ? '' : ' Windows 环境变更通知未完成，但路径已保存。'}`
        : '路径已经一致，无需修改。');
    } catch (e) {
      setRepairError(appError(e));
      try { setStatus(await detect()); } catch { setStatus(null); }
    } finally { pending.current = false; setBusy(false); setRepairing(false); }
  };
  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => { void proxyStartupIntent().then(intent => {
    if (intent === 'normal' || intent === 'admin') { setStartupIntent(intent); setTab('proxy'); }
  }).catch(() => { /* Desktop-required errors are displayed by the active page. */ }); }, []);
  useEffect(() => {
    const query = matchMedia('(prefers-color-scheme: dark)');
    const apply = () => { document.documentElement.dataset.theme = theme === 'system' ? query.matches ? 'dark' : 'light' : theme; };
    apply();
    query.addEventListener('change', apply);
    try { localStorage.setItem('theme', theme); } catch { /* Theme still applies if storage is unavailable. */ }
    return () => query.removeEventListener('change', apply);
  }, [theme]);
  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><img src="/guard.png" alt="" /><div><h1>Codex Guard</h1><p>桌面工作台</p></div></div>
      <p className="nav-label">工作空间</p>
    <nav className="tabs" aria-label="主导航">
      <button aria-current={tab === 'overview' ? 'page' : undefined} onClick={() => setTab('overview')}><Icon name="shield" />运行概览</button>
      <button aria-current={tab === 'diagnostics' ? 'page' : undefined} onClick={() => setTab('diagnostics')}><Icon name="check" />安装诊断</button>
      <button aria-current={tab === 'proxy' ? 'page' : undefined} onClick={() => setTab('proxy')}><Icon name="chevron" />代理启动器</button>
    </nav>
    <div className="sidebar-bottom"><p><span className="local-dot" />本地工作空间</p><small>启动检查与代理管理</small><button onClick={() => setSettingsOpen(true)}><Icon name="settings" />设置</button></div>
    </aside>
    <div className="workspace">
    <header className="app-header"><div><h2>{tab === 'overview' ? '运行概览' : tab === 'diagnostics' ? '安装诊断' : '代理启动'}</h2><p>{tab === 'overview' ? '安装状态与启动路径，在一个地方掌握。' : tab === 'diagnostics' ? '查看当前用户的 Codex 安装与 CLI 信息。' : '管理代理配置、进程与启动日志。'}</p></div><button disabled={busy} onClick={() => tab === 'proxy' ? setProxyRefresh(n => n + 1) : void refresh()}><Icon name="refresh" className={busy ? 'spinning' : ''} />{busy ? '正在检查…' : '刷新'}</button></header>
    <main aria-busy={busy}>
      {tab !== 'proxy' && repairing && <section className="card" role="status">正在修复并核验启动路径…</section>}
      {tab !== 'proxy' && repairNotice && <section className="card" role="status">{repairNotice}</section>}
      {tab !== 'proxy' && repairError && <section className="card error-card" role="alert"><h2>{repairError.message}</h2><p>{repairError.detail}</p><code>{repairError.code}</code><div className="card-actions"><button disabled={busy} onClick={() => void refresh()}>重新检测</button></div></section>}
      <div className="sr-only" role="status">{busy ? '正在检查 Codex 启动状态' : error ? error.message : status ? `检查完成： ${status.health.replace(/_/g, ' ')}` : ''}</div>
      {tab !== 'proxy' && error && <section className="card error-card" role="alert"><p className="section-label">暂时无法检测</p><h2><span className="status-dot danger" />{error.message}</h2><p>{error.detail}</p><code>{error.code}</code><div className="card-actions"><button onClick={() => void refresh()} disabled={busy}>重试</button></div></section>}
      {tab !== 'proxy' && !status && !error && !repairError && <section className="card health-card loading"><p className="section-label">Codex 启动状态</p><h2><span className="status-dot neutral" />正在检查安装…</h2><p className="health-description">正在读取当前 Windows 用户的应用包和 CLI 路径。</p></section>}
      {tab !== 'proxy' && status && (tab === 'overview' ? <Overview status={status} busy={busy} onRepair={() => setRepairTarget(status)} showDiagnostics={() => setTab('diagnostics')} /> : <Diagnostics status={status} />)}
      {tab === 'proxy' && <ProxyLauncher refreshKey={proxyRefresh} startupIntent={startupIntent} onIntentHandled={() => setStartupIntent(null)} />}
    </main>
    <footer><span>{busy ? '正在处理…' : status ? `上次检查： ${new Date(status.checked_at_unix_ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}` : '尚未完成检查'}</span><span>{tab === 'proxy' ? '仅对启动的进程设置代理' : '检测与用户级路径修复'}</span></footer>
    </div>
    {settingsOpen && <Settings theme={theme} setTheme={setTheme} close={() => setSettingsOpen(false)} />}
    {repairTarget && <ConfirmDialog title="修复启动路径" confirmLabel="确认修复" onConfirm={() => void runRepair()} onCancel={() => setRepairTarget(null)}>
      <p>将当前 Windows 用户的 CODEX_CLI_PATH 保存为当前 Codex 内置 CLI 路径，无需管理员权限。</p>
      <dl className="path-list"><div><dt>当前路径</dt><dd><code>{repairTarget.current_cli_path || '未设置'}</code></dd></div><div><dt>修复为</dt><dd><code>{repairTarget.expected_cli_path}</code></dd></div></dl>
      <p>已运行的 Codex 和终端需要重新打开；本次操作不会结束进程。</p>
    </ConfirmDialog>}
  </div>;
}

