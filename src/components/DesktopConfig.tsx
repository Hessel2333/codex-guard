import { useCallback, useEffect, useRef, useState } from 'react';
import { appError } from '../lib/api';
import { applyDesktop, desktopKeys, desktopStatus, launchDesktop, previewDesktop, previewDesktopRestore, restoreDesktop, verifyDesktop, type DesktopEdit, type DesktopKey, type DesktopPreview, type DesktopProbe, type DesktopSelection, type DesktopStatus, type DesktopLaunchRequest } from '../lib/desktop-api';
import type { AppError } from '../types';
import { ConfirmDialog } from './ConfirmDialog';
import { Icon } from './Icon';

const labels: Record<DesktopKey, string> = { developer_instructions: '自定义指令', model_reasoning_effort: '推理强度', approval_policy: '操作确认', sandbox_mode: '文件访问范围' };
const options: Partial<Record<DesktopKey, [string, string][]>> = {
  model_reasoning_effort: [['none', '关闭'], ['minimal', '最少'], ['low', '低'], ['medium', '中'], ['high', '高'], ['xhigh', '最高']],
  approval_policy: [['on-request', '按需确认'], ['never', '不弹出操作确认']],
  sandbox_mode: [['read-only', '只读'], ['workspace-write', '可写工作区'], ['danger-full-access', '完全访问']],
};
function valueLabel(key: DesktopKey, value: string | null) {
  return value === null ? '未设置，沿用默认' : options[key]?.find(([id]) => id === value)?.[1] ?? value;
}
function pathLabel(value: string) { return value.replace(/^\\\\\?\\UNC\\/i, '\\\\').replace(/^\\\\\?\\/, ''); }
function ErrorNotice({ error }: { error: AppError }) {
  return <div className="inline-error" role="alert"><strong>{error.message}</strong><details><summary>查看详情</summary><p>{error.detail}</p><code>{error.code}</code></details></div>;
}
type Pending = { preview: DesktopPreview; edits: DesktopEdit[]; selection: DesktopSelection };

export function DesktopConfig({ refreshKey, onBusyChange, onDirtyChange }: { refreshKey: number; onBusyChange: (busy: boolean) => void; onDirtyChange: (dirty: boolean) => void }) {
  const [selection, setSelection] = useState<DesktopSelection>({ home: null });
  const [homeDraft, setHomeDraft] = useState('');
  const [nextSelection, setNextSelection] = useState<DesktopSelection | null>(null);
  const [status, setStatus] = useState<DesktopStatus | null>(null);
  const [edits, setEdits] = useState<Partial<Record<DesktopKey, string | null>>>({});
  const [error, setError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState('');
  const [proof, setProof] = useState<DesktopProbe | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [useProxy, setUseProxy] = useState(true);
  const [launchPlan, setLaunchPlan] = useState<{ request: DesktopLaunchRequest; executable: string; processCount: number } | null>(null);
  const working = useRef(false);
  const mounted = useRef(true);
  const dirty = Object.keys(edits).length > 0;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; onBusyChange(false); onDirtyChange(false); }; }, [onBusyChange, onDirtyChange]);
  useEffect(() => { onBusyChange(!!busy); }, [busy, onBusyChange]);
  useEffect(() => { onDirtyChange(dirty); }, [dirty, onDirtyChange]);
  const load = useCallback(async () => {
    const next = await desktopStatus(selection);
    if (mounted.current) { setStatus(next); setHomeDraft(pathLabel(next.home)); }
    return next;
  }, [selection]);
  const perform = useCallback(async (message: string, task: () => Promise<void>) => {
    if (working.current) return;
    working.current = true; setBusy(message); setError(null); setNotice('');
    try { await task(); }
    catch (e) { if (mounted.current) setError(appError(e)); }
    finally { working.current = false; if (mounted.current) setBusy(''); }
  }, []);
  useEffect(() => { void perform('正在读取桌面配置…', async () => { await load(); }); }, [load, perform, refreshKey]);
  const current = (key: DesktopKey) => status?.settings.find(setting => setting.key === key);
  const draft = (key: DesktopKey) => Object.prototype.hasOwnProperty.call(edits, key) ? edits[key] ?? null : current(key)?.value ?? null;
  const change = (key: DesktopKey, value: string | null) => { setEdits(previous => ({ ...previous, [key]: value })); setNotice(''); };
  const editable = !!status?.target && status.revision !== null && !status.config_error && !status.external_manager;
  const verified = proof && proof.home === status?.home && proof.revision === status?.revision && proof.target_identity === status.target?.identity;
  const selectedEdits = (): DesktopEdit[] => desktopKeys.filter(key => Object.prototype.hasOwnProperty.call(edits, key)).map(key => ({ key, value: edits[key] ?? null }));
  const switchSelection = (next: DesktopSelection) => {
    if (dirty) { setNextSelection(next); return; }
    setStatus(null); setProof(null); setSelection(next);
  };
  const confirm = () => {
    if (!pending) return;
    const action = pending; setPending(null);
    void perform(action.preview.operation === 'apply' ? '正在检查兼容性并保存配置…' : '正在检查兼容性并恢复配置…', async () => {
      const result = action.preview.operation === 'apply'
        ? await applyDesktop(action.selection, action.preview, action.edits)
        : await restoreDesktop(action.selection, action.preview);
      setEdits({}); setProof(result.verification);
      setNotice(result.receipt.changed
        ? `${action.preview.operation === 'apply' ? '配置已保存' : '配置已恢复'}，并已通过兼容性检查。请在方便时重新打开 Codex，再新建会话；当前任务不会被中断。`
        : '配置没有变化，无需写入。');
      await load();
    });
  };
  return <div className="desktop-config-page" aria-busy={!!busy}>
    {busy && <div className="card operation-notice" role="status"><Icon name="refresh" className="spinning" />{busy}</div>}
    {notice && <div className="card operation-notice" role="status"><Icon name="check" />{notice}</div>}
    {error && <ErrorNotice error={error} />}
    <section className="card" aria-labelledby="desktop-instance-title">
      <div className="section-heading"><h2 id="desktop-instance-title">桌面实例</h2><button disabled={!!busy || !status?.target || status.revision === null || !!status.config_error || dirty} onClick={() => void perform('正在检查内置引擎兼容性…', async () => {
        if (status) { setProof(await verifyDesktop(selection, status)); setNotice('兼容性检查通过。运行中的会话和云端任务仍需单独确认。'); }
      })}>检查兼容性</button></div>
      {status?.target_error && <ErrorNotice error={status.target_error} />}
      <dl className="path-list desktop-paths">
        <div><dt>桌面程序</dt><dd><code>{status?.target ? pathLabel(status.target.executable) : status ? '未找到桌面安装' : '正在读取…'}</code></dd></div>
        <div><dt>内置引擎</dt><dd><code>{status?.target ? pathLabel(status.target.cli_path) : '尚未确认'}</code></dd></div>
        <div><dt>配置文件</dt><dd><code>{status ? pathLabel(status.config_path) : '正在读取…'}</code></dd></div>
      </dl>
      {status && <p className="desktop-scope">{status.target?.desktop_version ? `桌面版本 ${status.target.desktop_version} · ` : ''}{status.home_source === 'selected' ? '手动选择的目录' : status.home_source === 'environment' ? '来自 Guard 启动环境的目录' : '当前用户的默认目录'} · {status.target?.running_pids.length ?? 0} 个路径匹配的相关进程{status.target?.unverified_processes ? ` · ${status.target.unverified_processes} 个进程无法确认` : ''}</p>}
      <details className="desktop-location"><summary>选择其他配置目录</summary><label htmlFor="desktop-home">Codex 配置目录</label><input id="desktop-home" value={homeDraft} onChange={e => setHomeDraft(e.target.value)} disabled={!!busy} spellCheck={false} placeholder="填写完整目录路径" /><div className="proxy-actions"><button disabled={!!busy || !homeDraft.trim()} onClick={() => switchSelection({ home: homeDraft.trim() })}>读取此目录</button><button disabled={!!busy} onClick={() => switchSelection({ home: null })}>使用自动选择</button></div></details>
      <p className="desktop-scope">此目录可能同时供桌面端和 CLI 使用。尚未确认运行中的 Codex 是否使用此目录，请核对路径后再保存。</p>
      <div className="desktop-launch-options"><label><input type="checkbox" checked={useProxy} disabled={!!busy} onChange={e => setUseProxy(e.target.checked)} />使用已保存的代理</label><button className="primary-button" disabled={!!busy || dirty || !status?.target || !status.revision || !!status.config_error} onClick={() => {
        if (status?.target && status.revision) setLaunchPlan({ request: { selection: { home: status.home }, revision: status.revision, target_identity: status.target.identity, use_proxy: useProxy, confirmed: true }, executable: status.target.executable, processCount: status.target.running_pids.length });
      }}>启动此配置</button></div>
      <p className="desktop-scope">当前目录中的指令配置会保留，代理可同时使用。代理地址取自「代理启动器」中已保存的设置；启动前请先保存配置修改。</p>
    </section>
    <section className="card" aria-labelledby="desktop-diagnostics-title">
      <div className="section-heading"><h2 id="desktop-diagnostics-title">配置状态</h2><span className="muted">{verified ? proof.evidence.cli_version : '尚未完成兼容性检查'}</span></div>
      {status?.config_error && <ErrorNotice error={status.config_error} />}
      {status?.external_manager && <div className="desktop-warning" role="alert">此配置正由 codex-purge 管理。请先在原工具中恢复受管配置，再使用 Guard 修改。</div>}
      <dl className="desktop-checks">
        <div><dt>配置文件</dt><dd>{!status ? '正在读取' : status.config_error ? '无法解析或读取' : status.config_exists ? '可读取，格式有效' : '尚未创建，保存时创建'}</dd></div>
        <div><dt>内置引擎兼容性</dt><dd>{verified ? '常规配置项已通过隔离检查' : '待检查'}</dd></div>
        <div><dt>当前会话</dt><dd>尚未验证；不能据此判断云端任务已生效</dd></div>
      </dl>
    </section>
    <section className="card" aria-labelledby="desktop-editor-title">
      <div className="section-heading"><h2 id="desktop-editor-title">指令与运行配置</h2><span className="muted">{dirty ? '有未保存的修改' : '仅修改明确选择的配置项'}</span></div>
      <fieldset className="desktop-editor" disabled={!!busy || !editable}>
        <label htmlFor="desktop-instructions">自定义指令</label>
        {current('developer_instructions')?.advanced && <p className="desktop-scope">当前为高级配置，输入新指令后才会替换。</p>}
        <textarea id="desktop-instructions" value={draft('developer_instructions') ?? ''} onChange={e => change('developer_instructions', e.target.value)} rows={7} maxLength={131072} placeholder="填写你的工作偏好、输出要求或项目约定" />
        <div className="desktop-editor-meta"><small>替换此目录中的用户级自定义指令，不修改项目内的 AGENTS.md。</small><button onClick={() => change('developer_instructions', null)}>移除自定义指令</button></div>
        <div className="desktop-options">{(['model_reasoning_effort', 'approval_policy', 'sandbox_mode'] as DesktopKey[]).map(key => <label key={key}>{labels[key]}<select aria-label={labels[key]} value={Object.prototype.hasOwnProperty.call(edits, key) ? edits[key] ?? '__remove' : '__keep'} onChange={e => {
          if (e.target.value === '__keep') setEdits(previous => { const next = { ...previous }; delete next[key]; return next; });
          else change(key, e.target.value === '__remove' ? null : e.target.value);
        }}><option value="__keep">保持当前（{current(key)?.advanced ? '高级配置' : valueLabel(key, current(key)?.value ?? null)}）</option><option value="__remove">移除用户级设置</option>{options[key]?.map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>)}</div>
      </fieldset>
      <p className="desktop-scope">项目配置、会话选项及组织要求可能覆盖这里的设置。“不弹出操作确认”不会自动扩大文件访问范围。</p>
      <div className="form-footer"><button disabled={!!busy || !dirty} onClick={() => { setEdits({}); setNotice('未保存的修改已撤销。'); }}>撤销未保存修改</button><button className="primary-button" disabled={!!busy || !dirty || !editable} onClick={() => void perform('正在生成变更预览…', async () => {
        if (!status) return;
        const list = selectedEdits(); const preview = await previewDesktop(selection, status, list);
        if (!preview.changes.length) { setNotice('所选配置与当前文件一致，无需修改。'); return; }
        setPending({ preview, edits: list, selection: { ...selection } });
      })}>预览变更</button></div>
    </section>
    <section className="card" aria-labelledby="desktop-history-title">
      <div className="section-heading"><h2 id="desktop-history-title">备份与恢复</h2><span className="muted">最近 {status?.history.length ?? 0} 次操作</span></div>
      {status?.history_error && <ErrorNotice error={status.history_error} />}
      {!status?.history.length ? <p className="desktop-scope">暂无备份。首次保存前会自动创建快照；恢复操作本身也会保留快照。</p> : <ul className="desktop-history">{status.history.map(entry => <li key={entry.id}><div><strong>{entry.operation === 'restore' ? '恢复配置' : entry.operation === 'apply' ? '保存配置' : '未识别的记录'}</strong><time>{entry.created_at ? new Date(entry.created_at).toLocaleString() : '时间未知'}</time><p>{entry.note}</p>{entry.state === 'prepared' && <span className="desktop-scope">操作未完成</span>}</div><button disabled={!!busy || dirty || !editable || !entry.can_restore} onClick={() => void perform('正在核对备份与当前配置…', async () => {
        if (status) setPending({ preview: await previewDesktopRestore(selection, status, entry.id), edits: [], selection: { ...selection } });
      })}>预览恢复</button></li>)}</ul>}
      <details className="desktop-location"><summary>备份存放位置</summary><code>{status ? pathLabel(status.backup_path) : '尚未读取'}</code><p className="desktop-scope">备份仅保存在本机，可能包含原配置中的敏感信息，请勿公开分享。</p></details>
    </section>
    {pending && <ConfirmDialog title={pending.preview.operation === 'apply' ? '确认配置变更' : '确认恢复配置'} confirmLabel={pending.preview.operation === 'apply' ? '验证并应用' : '验证并恢复'} onConfirm={confirm} onCancel={() => setPending(null)}>
      <p>将修改以下目录的用户配置，写入前会检查兼容性并保留快照。不会关闭 Codex 或中断当前任务。</p><code>{pathLabel(pending.preview.home)}</code>
      <div className="desktop-diff">{pending.preview.changes.map(item => <div key={item.key}><strong>{labels[item.key]}</strong><span>当前</span><pre>{valueLabel(item.key, item.before)}</pre><span>修改后</span><pre>{valueLabel(item.key, item.after)}</pre></div>)}</div>
      <p>如果配置已被其他操作修改，本次保存会停止，不会覆盖新内容。</p>
    </ConfirmDialog>}
    {nextSelection && <ConfirmDialog title="切换配置目录" confirmLabel="放弃修改并切换" onCancel={() => setNextSelection(null)} onConfirm={() => { setEdits({}); setStatus(null); setProof(null); setSelection(nextSelection); setNextSelection(null); }}><p>当前有未保存的修改。切换目录会放弃这些修改，已保存的文件不会改变。</p></ConfirmDialog>}
    {launchPlan && <ConfirmDialog title="使用当前配置启动 Codex" confirmLabel="确认启动" onCancel={() => setLaunchPlan(null)} onConfirm={() => {
      const plan = launchPlan; setLaunchPlan(null);
      void perform('正在使用所选配置启动 Codex…', async () => {
        const receipt = await launchDesktop(plan.request);
        setNotice(`Codex 已使用此配置启动${receipt.proxy_enabled ? '，同时使用已保存的代理' : '，未注入 Guard 代理'} · PID ${receipt.pid}`);
        await load();
      });
    }}><p>{launchPlan.processCount ? `将关闭并替换 ${launchPlan.processCount} 个路径匹配的相关进程，未保存的工作和正在执行的任务可能中断。` : '将使用所选配置目录启动 Codex。'}</p><dl className="path-list"><div><dt>配置目录</dt><dd><code>{pathLabel(launchPlan.request.selection.home ?? '')}</code></dd></div><div><dt>代理</dt><dd>{launchPlan.request.use_proxy ? '同时使用已保存的代理' : '不注入 Guard 代理，系统代理仍可能生效'}</dd></div></dl><code>{pathLabel(launchPlan.executable)}</code><p>本次不会修改或移除现有指令配置。</p></ConfirmDialog>}
  </div>;
}
