import { useEffect, useRef } from 'react';
import { Icon } from './Icon';
import type { Updates } from '../lib/useUpdates';
export type Theme = 'system' | 'light' | 'dark';

export function Settings({ theme, setTheme, close, updates }: { theme: Theme; setTheme: (theme: Theme) => void; close: () => void; updates: Updates }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} onClose={close} onClick={e => { if (e.target === e.currentTarget) dialog.current?.close(); }} aria-labelledby="settings-title">
    <div className="dialog-heading"><h2 id="settings-title">设置</h2><button aria-label="关闭设置" onClick={() => dialog.current?.close()}><Icon name="close" /></button></div>
    <label className="theme-setting">外观<select value={theme} onChange={e => setTheme(e.target.value as Theme)}><option value="system">跟随 Windows</option><option value="light">浅色</option><option value="dark">深色</option></select></label>
    <section className="update-settings" aria-labelledby="update-title"><h3 id="update-title">应用更新</h3>
      <p>当前版本 {updates.version}</p>
      <label className="update-toggle"><input type="checkbox" checked={updates.automatic} onChange={e => updates.setAutomatic(e.target.checked)} />自动检查更新</label>
      <p className="settings-note">启动时及每 6 小时检查。安装更新前由你确认，安装时将关闭 Codex Guard。</p>
      <div className="card-actions"><button disabled={updates.phase !== 'idle'} onClick={() => void updates.checkNow()}>检查更新</button><button onClick={() => { close(); updates.openNotes(); }}>查看本次更新内容</button></div>
      {updates.phase !== 'idle' && <p role="status">{updates.phase === 'checking' ? '正在检查更新…' : updates.phase === 'installing' ? '正在校验并启动安装程序…' : `正在下载更新…${updates.progress === null ? '' : ` ${updates.progress}%`}`}</p>}
      {updates.phase === 'downloading' && <progress max={100} value={updates.progress ?? undefined} aria-label="更新下载进度" />}
      {updates.message && <p role="status">{updates.message}</p>}
      {updates.error && <p className="update-error" role="alert">{updates.error}</p>}
      {updates.available && <div className="update-available"><h4>新版本 {updates.available.version}</h4><div className="release-notes">{updates.available.body || '此版本未提供更新说明。'}</div><button className="primary-button" disabled={updates.phase !== 'idle'} onClick={() => void updates.install()}>下载并安装更新</button></div>}
    </section>
    <p className="settings-note">Codex Guard · {updates.version}<br />启动诊断与代理启动器</p>
  </dialog>;
}
