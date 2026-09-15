import { useEffect, useRef } from 'react';
import { Icon } from './Icon';
export type Theme = 'system' | 'light' | 'dark';

export function Settings({ theme, setTheme, close }: { theme: Theme; setTheme: (theme: Theme) => void; close: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} onClose={close} onClick={e => { if (e.target === e.currentTarget) dialog.current?.close(); }} aria-labelledby="settings-title">
    <div className="dialog-heading"><h2 id="settings-title">设置</h2><button aria-label="关闭设置" onClick={() => dialog.current?.close()}><Icon name="close" /></button></div>
    <label className="theme-setting">外观<select value={theme} onChange={e => setTheme(e.target.value as Theme)}><option value="system">跟随 Windows</option><option value="light">浅色</option><option value="dark">深色</option></select></label>
    <p className="settings-note">Codex Guard · 0.2.0<br />启动诊断与代理启动器</p>
  </dialog>;
}
