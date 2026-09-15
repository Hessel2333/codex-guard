import { useEffect, useRef, type ReactNode } from 'react';

export function ConfirmDialog({ title, children, confirmLabel, onConfirm, onCancel }: {
  title: string; children: ReactNode; confirmLabel: string; onConfirm: () => void; onCancel: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} className="confirmation" onCancel={e => { e.preventDefault(); onCancel(); }} aria-labelledby="confirm-title">
    <h2 id="confirm-title">{title}</h2><div className="confirm-body">{children}</div>
    <div className="card-actions"><button autoFocus onClick={onCancel}>取消</button><button className="primary-button" onClick={onConfirm}>{confirmLabel}</button></div>
  </dialog>;
}
