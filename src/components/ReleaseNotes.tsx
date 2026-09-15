import { useEffect, useRef } from 'react';
export function ReleaseNotes({ version, notes, close }: { version: string; notes: string; close: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} className="confirmation" aria-labelledby="release-title" onCancel={e => { e.preventDefault(); close(); }}>
    <h2 id="release-title">本次更新内容 · {version}</h2>
    <p>当前运行 Codex Guard {version}</p><div className="release-notes">{notes}</div>
    <div className="card-actions"><button className="primary-button" autoFocus onClick={close}>知道了</button></div>
  </dialog>;
}
