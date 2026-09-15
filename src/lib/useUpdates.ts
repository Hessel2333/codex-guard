import { useCallback, useEffect, useRef, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { check, type Update } from '@tauri-apps/plugin-updater';
import release from '../release.json';

export const AUTO_KEY = 'guard.autoUpdateCheck';
export const SEEN_KEY = 'guard.seenRelease';
function read(key: string): string | null { try { return localStorage.getItem(key); } catch { return null; } }
function save(key: string, value: string) { try { localStorage.setItem(key, value); } catch { /* Session state remains usable. */ } }

export function useUpdates() {
  const [automatic, setAutomaticState] = useState(() => read(AUTO_KEY) !== 'false');
  const [showNotes, setShowNotes] = useState(() => isTauri() && read(SEEN_KEY) !== release.version);
  const [available, setAvailable] = useState<Update | null>(null);
  const [phase, setPhase] = useState<'idle' | 'checking' | 'downloading' | 'installing'>('idle');
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [progress, setProgress] = useState<number | null>(null);
  const lock = useRef(false);
  const resource = useRef<Update | null>(null);
  const checkNow = useCallback(async () => {
    if (lock.current) return;
    if (!isTauri()) { setError('请在 Windows 桌面应用中检查更新。'); return; }
    lock.current = true; setPhase('checking'); setError(''); setMessage('');
    try {
      const update = await check({ timeout: 20000 });
      const old = resource.current;
      resource.current = update; setAvailable(update);
      if (old) void old.close().catch(() => {});
      setMessage(update ? `发现新版本 ${update.version}` : '当前已是最新版本。');
    } catch (e) { setError(`检查更新失败，请检查网络后重试。${String(e)}`); }
    finally { lock.current = false; setPhase('idle'); }
  }, []);
  useEffect(() => {
    if (!automatic || !isTauri()) return;
    const start = window.setTimeout(() => void checkNow(), 3000);
    const interval = window.setInterval(() => void checkNow(), 6 * 60 * 60 * 1000);
    return () => { clearTimeout(start); clearInterval(interval); };
  }, [automatic, checkNow]);
  const install = async () => {
    if (lock.current || !resource.current) return;
    lock.current = true; setPhase('downloading'); setProgress(null); setError(''); setMessage('');
    let total = 0, received = 0;
    try {
      await resource.current.downloadAndInstall(event => {
        if (event.event === 'Started') { total = event.data.contentLength ?? 0; received = 0; }
        if (event.event === 'Progress') { received += event.data.chunkLength; setProgress(total ? Math.min(100, Math.round(received / total * 100)) : null); }
        if (event.event === 'Finished') setPhase('installing');
      }, { timeout: 120000 });
      setMessage('安装程序已启动。安装完成后请重新打开 Codex Guard。');
    } catch (e) { setError(`更新未完成，可重试。${String(e)}`); }
    finally { lock.current = false; setPhase('idle'); }
  };
  return { automatic, setAutomatic: (value: boolean) => { save(AUTO_KEY, String(value)); setAutomaticState(value); },
    showNotes, closeNotes: () => { save(SEEN_KEY, release.version); setShowNotes(false); },
    openNotes: () => setShowNotes(true), available, phase, message, error, progress, checkNow, install,
    version: release.version, notes: release.notes };
}
export type Updates = ReturnType<typeof useUpdates>;
