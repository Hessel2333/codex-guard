import { useCallback, useEffect, useRef, useState } from 'react';
import type { AppError, ProxySettings, ProxyStatus, ProxyLogs } from '../types';
import { appError } from '../lib/api';
import { dateTime } from '../lib/presentation';
import { getProxyStatus, getProxyLogs, saveProxySettings, launchProxy, stopProxy, installProxyShortcuts } from '../lib/proxy-api';
import { ConfirmDialog } from './ConfirmDialog';
import { Icon } from './Icon';

type Action = 'launch' | 'admin' | 'restart' | 'stop';
const labels: Record<Action, string> = { launch: '使用代理启动', admin: '以管理员身份启动', restart: '使用代理重启', stop: '停止 Codex' };
const defaults: ProxySettings = { http_proxy: 'http://127.0.0.1:7890', all_proxy: 'socks5://127.0.0.1:7890', no_proxy: 'localhost,127.0.0.1,::1', app_path: null };

export function ProxyLauncher({ refreshKey, startupIntent, onIntentHandled }: { refreshKey: number; startupIntent: string | null; onIntentHandled: () => void }) {
  const [status, setStatus] = useState<ProxyStatus | null>(null);
  const [settings, setSettings] = useState<ProxySettings>(defaults);
  const [logs, setLogs] = useState<ProxyLogs | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [logError, setLogError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState('');
  const [dirty, setDirty] = useState(false);
  const [action, setAction] = useState<Action | null>(null);
  const [live, setLive] = useState(true);
  const [logTab, setLogTab] = useState<'current' | 'legacy'>('current');
  const initialized = useRef(false);
  const intentHandled = useRef(false);
  const running = useRef(false);
  const reading = useRef<Promise<void> | null>(null);

  const refresh = useCallback(async (forceFresh = false) => {
    if (reading.current) { await reading.current; if (!forceFresh) return; }
    const task = (async () => {
      const [state, history] = await Promise.allSettled([getProxyStatus(), getProxyLogs()]);
      if (state.status === 'fulfilled') {
        setStatus(state.value);
        if (!initialized.current) { setSettings(state.value.settings); initialized.current = true; }
      } else { setError(appError(state.reason)); setStatus(null); }
      if (history.status === 'fulfilled') { setLogs(history.value); setLogError(null); }
      else setLogError(appError(history.reason));
    })();
    reading.current = task;
    try { await task; } finally { if (reading.current === task) reading.current = null; }
  }, []);
  useEffect(() => { void refresh(); }, [refresh, refreshKey]);
  useEffect(() => {
    if (!live) return;
    const timer = setInterval(() => { if (!running.current) void refresh(); }, 5000);
    return () => clearInterval(timer);
  }, [live, refresh]);
  useEffect(() => {
    if (status && !intentHandled.current && (startupIntent === 'normal' || startupIntent === 'admin')) {
      intentHandled.current = true;
      setAction(startupIntent === 'admin' ? 'admin' : 'launch');
      onIntentHandled();
    }
  }, [startupIntent, status, onIntentHandled]);

  async function perform(message: string, job: () => Promise<string>) {
    if (running.current) return;
    running.current = true;
    setBusy(message); setError(null); setNotice('');
    try { setNotice(await job()); }
    catch (e) { setError(appError(e)); }
    finally { await refresh(true); running.current = false; setBusy(''); }
  }
  function edit(key: keyof ProxySettings, value: string) {
    setSettings(previous => ({ ...previous, [key]: key === 'app_path' && value === '' ? null : value }));
    setDirty(true); setNotice('');
  }
  function confirm() {
    const selected = action;
    setAction(null);
    if (!selected) return;
    void perform(selected === 'admin' ? '正在等待 Windows 管理员授权和启动…' : `${labels[selected]}…`, async () => {
      if (selected === 'stop') { const pids = await stopProxy(); return pids.length ? `已停止 ${pids.length} 个已确认的 Codex 进程。` : '没有正在运行的已确认 Codex 进程。'; }
      const receipt = await launchProxy(selected === 'admin');
      return `Codex 已使用代理启动 · PID ${receipt.pid} · ${receipt.administrator ? '管理员' : '普通用户'}`;
    });
  }
  const managed = status?.processes.filter(p => p.managed) ?? [];
  const lastLaunch = status?.last_launch;
  const knownProxySession = lastLaunch && managed.some(p => p.pid === lastLaunch.pid && p.started_unix_ms === lastLaunch.started_unix_ms);
  const actionDisabled = !!busy || dirty || !status?.executable;
  return <div className="proxy-page">
    <div className="page-heading"><div><h2>代理启动器</h2><p>为 Codex 进程设置代理，无需 TUN，不修改系统代理。</p></div><label className="live-toggle"><input type="checkbox" checked={live} onChange={e => setLive(e.target.checked)} />自动刷新状态和日志</label></div>
    {error && <div className="inline-error" role="alert"><strong>{error.message}</strong><p>{error.detail}</p><code>{error.code}</code><button disabled={!!busy} onClick={() => { setError(null); void refresh(); }}>重新检测</button></div>}
    {notice && <p className="operation-notice success" role="status">{notice}</p>}
    {busy && <p className="operation-notice" role="status"><Icon name="refresh" className="spinning" />{busy}</p>}
    <section className="card proxy-session" aria-labelledby="proxy-session-title">
      <div><p className="section-label">Codex 会话</p><h2 id="proxy-session-title"><span className={`status-dot ${managed.length ? 'success' : 'neutral'}`} />{!status ? error ? '暂时无法检测' : '正在检测 Codex…' : managed.length ? 'Codex 正在运行' : status.executable ? '可以启动' : '未检测到 Codex'}</h2><p className="session-detail">{knownProxySession ? '此会话由 Boot Guard 使用代理配置启动。' : managed.length ? '检测到现有会话，无法确认该会话的代理配置。' : status?.executable ? '已保存的配置将应用到新启动的 Codex 进程。' : '请在下方设置 Codex 桌面程序路径以继续。'}</p></div>
      <div className="proxy-actions"><button className="primary-button" disabled={actionDisabled} onClick={() => setAction('launch')}>使用代理启动</button><button disabled={actionDisabled} onClick={() => setAction('admin')}><Icon name="shield" />管理员启动</button><button disabled={actionDisabled || !managed.length} onClick={() => setAction('restart')}>重启</button><button className="danger-button" disabled={actionDisabled || !managed.length} onClick={() => setAction('stop')}>停止 Codex</button></div>
      {status?.discovery_error && <p className="discovery-error">{status.discovery_error.message}: {status.discovery_error.detail}</p>}
      <p className="detected-exe"><span>桌面程序路径</span><code>{status?.executable ?? '未检测到'}</code></p>
    </section>
    <section className="card" aria-labelledby="proxy-settings-title">
      <div className="section-heading"><h2 id="proxy-settings-title">代理设置</h2><span className="muted">{dirty ? '有未保存的修改' : '下次启动时生效'}</span></div>
      <form onSubmit={e => { e.preventDefault(); void perform('正在保存代理设置…', async () => { const saved = await saveProxySettings(settings); setSettings(saved); setDirty(false); initialized.current = true; return '代理设置已保存，系统代理保持不变。'; }); }}>
        <fieldset disabled={!!busy} className="proxy-form">
          <label>HTTP / HTTPS 代理<input value={settings.http_proxy} onChange={e => edit('http_proxy', e.target.value)} spellCheck={false} required /><small>HTTP_PROXY 与 HTTPS_PROXY</small></label>
          <label>SOCKS / ALL_PROXY 代理<input value={settings.all_proxy} onChange={e => edit('all_proxy', e.target.value)} spellCheck={false} required /><small>ALL_PROXY · socks5 / socks5h / http</small></label>
          <label className="full-width">不使用代理的地址<input value={settings.no_proxy} onChange={e => edit('no_proxy', e.target.value)} spellCheck={false} /><small>NO_PROXY · 多个地址以逗号分隔</small></label>
          <label className="full-width">手动指定桌面程序路径<input value={settings.app_path ?? ''} onChange={e => edit('app_path', e.target.value)} placeholder="自动查找（推荐）" spellCheck={false} /><small>可选：填写 ChatGPT.exe 或 Codex.exe 的完整路径，留空则自动查找。</small></label>
        </fieldset>
        <div className="form-footer"><p>启动或重启 Codex 前，请先保存修改。</p><button type="submit" disabled={!!busy || !dirty}>保存设置</button></div>
      </form>
    </section>
    <section className="card" aria-labelledby="proxy-process-title">
      <div className="section-heading"><h2 id="proxy-process-title">进程</h2><span className="muted">{managed.length} 个已确认进程 · {status?.administrator ? 'Guard 已获得管理员权限' : '普通用户'}</span></div>
      {!status?.processes.length ? <p className="muted">当前 Windows 会话中未检测到相关进程。</p> : <div className="process-list">{status.processes.map(p => <div className="process-row" key={`${p.pid}-${p.started_unix_ms}`}><div className="process-summary"><strong>{p.name}</strong><span>PID {p.pid}</span><span>{p.managed ? '已确认' : '未纳入管理'}</span><span>{dateTime(p.started_unix_ms)}</span></div><code>{p.path ?? p.error ?? '无法读取路径'}</code>{p.error && p.path && <p className="danger">{p.error}</p>}</div>)}</div>}
    </section>
    <section className="card" aria-labelledby="proxy-logs-title">
      <div className="section-heading"><h2 id="proxy-logs-title">代理日志</h2><div className="log-tabs"><button aria-pressed={logTab === 'current'} onClick={() => setLogTab('current')}>Boot Guard</button><button aria-pressed={logTab === 'legacy'} onClick={() => setLogTab('legacy')}>原启动器</button></div></div>
      {logError ? <p role="alert" className="danger">{logError.message}: {logError.detail}</p> : <pre className="log-view">{logs?.[logTab] || '暂无日志。'}</pre>}
      <p className="log-path"><code>{logs ? logTab === 'current' ? logs.current_path : logs.legacy_path : '正在读取日志路径…'}</code></p>
    </section>
    <section className="card shortcut-card"><div><h2>桌面快捷方式</h2><p>创建普通启动和管理员启动快捷方式，使用带角标的 Codex 图标。打开后确认启动。</p></div><button disabled={!!busy || dirty} onClick={() => void perform('正在创建桌面快捷方式…', async () => { const paths = await installProxyShortcuts(); return `已创建快捷方式： ${paths.join(' · ')}`; })}>创建或更新快捷方式</button></section>
    {action && <ConfirmDialog title={`${labels[action]}?`} confirmLabel={action === 'stop' ? '停止已确认的进程' : '确认启动'} onConfirm={confirm} onCancel={() => setAction(null)}>
      <p>{managed.length ? `${managed.length} 个已确认的 Codex 进程将关闭，未保存的工作和正在执行的任务可能中断。` : 'Codex 将使用已保存的代理设置启动。'}</p>
      {managed.length > 0 && <p className="confirm-processes">{managed.map(p => `${p.name} · ${p.pid}`).join(', ')}</p>}
      {action === 'admin' && <p>Windows 将通过 UAC 请求管理员授权，请使用同一个 Windows 账号。Boot Guard 主窗口的权限保持不变。</p>}
      <code>{status?.executable}</code>
    </ConfirmDialog>}
  </div>;
}

