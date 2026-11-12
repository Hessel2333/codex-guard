use super::config::{self, err};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn unique() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    config::plain_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| err("DESKTOP_PATH", "配置路径无效", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| err("DESKTOP_WRITE", "无法创建配置目录", e))?;
    config::plain_path(path)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".guard-config-")
        .tempfile_in(parent)
        .map_err(|e| err("DESKTOP_WRITE", "无法创建临时文件", e))?;
    tmp.write_all(bytes)
        .and_then(|_| tmp.as_file().sync_all())
        .map_err(|e| err("DESKTOP_WRITE", "无法保存临时文件", e))?;
    let tmp = tmp.into_temp_path();
    for attempt in 0..5 {
        #[cfg(windows)]
        let result = {
            use std::os::windows::ffi::OsStrExt;
            use windows::{
                core::PCWSTR,
                Win32::Storage::FileSystem::{ReplaceFileW, REPLACE_FILE_FLAGS},
            };
            if path
                .try_exists()
                .map_err(|e| err("DESKTOP_WRITE", "无法检查目标文件", e))?
            {
                let dest: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                let source: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
                // Preserve the destination's DACL and attributes on Windows.
                unsafe {
                    ReplaceFileW(
                        PCWSTR(dest.as_ptr()),
                        PCWSTR(source.as_ptr()),
                        None,
                        REPLACE_FILE_FLAGS(0),
                        None,
                        None,
                    )
                }
                .map_err(|e| std::io::Error::from_raw_os_error((e.code().0 as u32 & 0xffff) as i32))
            } else {
                fs::rename(&tmp, path)
            }
        };
        #[cfg(not(windows))]
        let result = {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(path) {
                fs::set_permissions(&tmp, meta.permissions())
                    .map_err(|e| err("DESKTOP_WRITE", "无法保留文件权限", e))?;
            } else {
                fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
                    .map_err(|e| err("DESKTOP_WRITE", "无法设置文件权限", e))?;
            }
            fs::rename(&tmp, path)
        };
        match result {
            Ok(()) => return Ok(()),
            Err(e)
                if attempt < 4
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::WouldBlock
                    ) =>
            {
                std::thread::sleep(Duration::from_millis(40 * (attempt + 1)))
            }
            Err(e) => return Err(err("DESKTOP_COMMIT", "无法替换配置文件，备份已保留", e)),
        }
    }
    unreachable!()
}
fn json_write(path: &Path, value: &impl Serialize) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| err("DESKTOP_JOURNAL", "无法保存操作记录", e))?;
    atomic_write(path, &bytes)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    id: String,
    home: String,
    created_at: u64,
    operation: String,
    state: String,
    before_revision: String,
    after_revision: String,
    keys: Vec<String>,
}
type Snapshot = (Manifest, Option<Vec<u8>>, Option<Vec<u8>>);
#[derive(Debug, Clone, Serialize)]
pub struct History {
    pub id: String,
    pub created_at: u64,
    pub operation: String,
    pub state: String,
    pub can_restore: bool,
    pub note: String,
}
#[derive(Debug, Serialize)]
pub struct Receipt {
    pub changed: bool,
    pub snapshot_id: Option<String>,
    pub revision: String,
}
pub struct Store {
    pub home: PathBuf,
    pub root: PathBuf,
    #[cfg(test)]
    fail_after_replace: bool,
}
impl Store {
    pub fn new(home: PathBuf, data: &Path) -> Self {
        let identity = home.to_string_lossy().replace('/', "\\").to_lowercase();
        Self {
            home,
            root: data
                .join("desktop-config")
                .join(config::digest(identity.as_bytes())),
            #[cfg(test)]
            fail_after_replace: false,
        }
    }
    pub fn config_path(&self) -> PathBuf {
        self.home.join("config.toml")
    }
    pub fn current(&self) -> Result<Option<Vec<u8>>, AppError> {
        config::read(&self.config_path())
    }
    fn folder(&self, id: &str) -> Result<PathBuf, AppError> {
        if id.is_empty() || id.len() > 100 || !id.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            return Err(err(
                "DESKTOP_SNAPSHOT",
                "备份编号无效",
                "未读取目标目录之外的文件。",
            ));
        }
        let folder = self.root.join(id);
        config::plain_path(&folder)?;
        Ok(folder)
    }
    fn load(&self, id: &str) -> Result<Snapshot, AppError> {
        let folder = self.folder(id)?;
        let raw = config::read(&folder.join("entry.json"))?
            .ok_or_else(|| err("DESKTOP_SNAPSHOT", "备份记录不存在", id))?;
        let manifest: Manifest = serde_json::from_slice(&raw)
            .map_err(|_| err("DESKTOP_SNAPSHOT", "备份记录损坏", id))?;
        if manifest.schema != 1
            || manifest.id != id
            || manifest.home != self.home.to_string_lossy()
            || manifest.keys.is_empty()
            || manifest.keys.len() > config::KEYS.len()
            || manifest
                .keys
                .iter()
                .any(|k| !config::KEYS.contains(&k.as_str()))
            || manifest
                .keys
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != manifest.keys.len()
            || !["prepared", "committed"].contains(&manifest.state.as_str())
        {
            return Err(err(
                "DESKTOP_SNAPSHOT",
                "备份不属于当前配置目录或格式无效",
                id,
            ));
        }
        let before = config::read(&folder.join("before.toml"))?;
        let after = config::read(&folder.join("after.toml"))?;
        if config::revision(before.as_deref()) != manifest.before_revision
            || config::revision(after.as_deref()) != manifest.after_revision
        {
            return Err(err("DESKTOP_SNAPSHOT", "备份校验失败，未恢复", id));
        }
        Ok((manifest, before, after))
    }
    pub fn restore_bytes(
        &self,
        id: &str,
        current: Option<&[u8]>,
    ) -> Result<(Option<Vec<u8>>, Vec<String>), AppError> {
        let (manifest, before, after) = self.load(id)?;
        // An interrupted transaction that never reached publication owns nothing.
        if manifest.state == "prepared" && current == before.as_deref() {
            return Err(err(
                "DESKTOP_NOT_APPLIED",
                "这次操作尚未写入配置，无需恢复",
                id,
            ));
        }
        let restored =
            config::restore(current, before.as_deref(), after.as_deref(), &manifest.keys)?;
        Ok((restored, manifest.keys))
    }
    pub fn history(&self, current: Option<&[u8]>) -> Result<Vec<History>, AppError> {
        config::plain_path(&self.root)?;
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(err("DESKTOP_HISTORY", "无法读取备份列表", e)),
        };
        let mut ids = Vec::new();
        for entry in entries.take(4096) {
            let entry = entry.map_err(|e| err("DESKTOP_HISTORY", "无法读取备份列表", e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.is_empty()
                && name.len() <= 100
                && name.bytes().all(|b| b.is_ascii_digit() || b == b'-')
            {
                ids.push(name);
            }
        }
        // IDs start with nanoseconds since epoch, followed by PID and sequence.
        ids.sort_by_cached_key(|id| {
            std::cmp::Reverse(
                id.split('-')
                    .next()
                    .and_then(|s| s.parse::<u128>().ok())
                    .unwrap_or_default(),
            )
        });
        let mut result = Vec::new();
        for id in ids.into_iter().take(30) {
            match self.load(&id) {
                Ok((m, before, after)) => {
                    let planned = if m.state == "prepared" && current == before.as_deref() {
                        Err(err("DESKTOP_NOT_APPLIED", "尚未写入，无需恢复", ""))
                    } else {
                        config::restore(current, before.as_deref(), after.as_deref(), &m.keys)
                    };
                    let can_restore = planned.as_ref().is_ok_and(|b| b.as_deref() != current);
                    let note = match planned {
                        Ok(bytes) if bytes.as_deref() == current => "当前已是恢复后的内容".into(),
                        Ok(_) if m.state == "prepared" => "操作未完成，可从快照恢复".into(),
                        Ok(_) if config::revision(current) != m.after_revision => {
                            "保留其他修改，仅恢复本次变更项".into()
                        }
                        Ok(_) => "可完整恢复本次修改前的文件".into(),
                        Err(e) => e.message,
                    };
                    result.push(History {
                        id,
                        created_at: m.created_at,
                        operation: m.operation,
                        state: m.state,
                        can_restore,
                        note,
                    });
                }
                Err(e) => result.push(History {
                    id,
                    created_at: 0,
                    operation: "unknown".into(),
                    state: "invalid".into(),
                    can_restore: false,
                    note: e.message,
                }),
            }
        }
        Ok(result)
    }
    pub fn commit(
        &self,
        expected: &str,
        next: Option<&[u8]>,
        keys: Vec<String>,
        operation: &str,
    ) -> Result<Receipt, AppError> {
        let before = self.current()?;
        if config::revision(before.as_deref()) != expected {
            return Err(err(
                "DESKTOP_STALE",
                "配置已发生变化，请重新预览",
                "原文件未修改。",
            ));
        }
        if before.as_deref() == next {
            return Ok(Receipt {
                changed: false,
                snapshot_id: None,
                revision: expected.into(),
            });
        }
        if keys.is_empty() || keys.iter().any(|k| !config::KEYS.contains(&k.as_str())) {
            return Err(err("DESKTOP_EDITS", "变更项无效", "未写入配置。"));
        }
        let id = unique();
        let folder = self.folder(&id)?;
        fs::create_dir_all(&folder)
            .map_err(|e| err("DESKTOP_BACKUP", "无法创建备份，原文件未修改", e))?;
        config::plain_path(&folder)?;
        if let Some(bytes) = &before {
            atomic_write(&folder.join("before.toml"), bytes)?;
        }
        if let Some(bytes) = next {
            atomic_write(&folder.join("after.toml"), bytes)?;
        }
        let mut manifest = Manifest {
            schema: 1,
            id: id.clone(),
            home: self.home.to_string_lossy().into_owned(),
            created_at: now(),
            operation: operation.into(),
            state: "prepared".into(),
            before_revision: expected.into(),
            after_revision: config::revision(next),
            keys,
        };
        json_write(&folder.join("entry.json"), &manifest)?;
        // Flush and validate both immutable snapshots before publishing anything.
        self.load(&id)?;
        if config::revision(self.current()?.as_deref()) != expected {
            return Err(err(
                "DESKTOP_STALE",
                "配置在备份期间发生变化，请重新预览",
                "本次未覆盖其他操作的修改。",
            ));
        }
        if let Some(bytes) = next {
            atomic_write(&self.config_path(), bytes)?;
        } else {
            fs::remove_file(self.config_path())
                .map_err(|e| err("DESKTOP_COMMIT", "无法移除本次生成的配置文件", e))?;
        }
        if config::revision(self.current()?.as_deref()) != manifest.after_revision {
            return Err(err(
                "DESKTOP_VERIFY",
                "写入后的配置核验失败，快照已保留",
                &id,
            ));
        }
        #[cfg(test)]
        if self.fail_after_replace {
            return Err(err("TEST_INTERRUPTED", "模拟提交中断", id));
        }
        manifest.state = "committed".into();
        json_write(&folder.join("entry.json"), &manifest).map_err(|_| {
            err(
                "DESKTOP_PARTIAL",
                "配置已写入，但操作记录未完成",
                format!("快照 {id} 已保留，请刷新备份列表核对。"),
            )
        })?;
        Ok(Receipt {
            changed: true,
            snapshot_id: Some(id),
            revision: manifest.after_revision,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::{Edit, KEYS};
    use super::*;
    fn fixture() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir(&home).unwrap();
        let store = Store::new(home, &dir.path().join("guard"));
        (dir, store)
    }
    fn apply(store: &Store, text: &str) -> Receipt {
        let before = store.current().unwrap();
        let (next, changes) = config::patch(
            before.as_deref(),
            &[Edit {
                key: KEYS[0].into(),
                value: Some(text.into()),
            }],
        )
        .unwrap();
        store
            .commit(
                &config::revision(before.as_deref()),
                Some(&next),
                changes.iter().map(|c| c.key.clone()).collect(),
                "apply",
            )
            .unwrap()
    }
    #[test]
    fn exact_restore_bom_crlf_and_reversible_restore() {
        let (_dir, store) = fixture();
        let original = b"\xef\xbb\xbf# keep\r\nmodel = 'fixture'\r\n";
        fs::write(store.config_path(), original).unwrap();
        let applied = apply(&store, "test");
        let current = store.current().unwrap();
        let (next, keys) = store
            .restore_bytes(applied.snapshot_id.as_ref().unwrap(), current.as_deref())
            .unwrap();
        let restored = store
            .commit(&applied.revision, next.as_deref(), keys, "restore")
            .unwrap();
        assert_eq!(store.current().unwrap().unwrap(), original);
        let (undo, _) = store
            .restore_bytes(restored.snapshot_id.as_ref().unwrap(), Some(original))
            .unwrap();
        assert_eq!(undo, current);
    }
    #[test]
    fn repeated_apply_does_not_lose_ownership_or_user_edits() {
        let (_dir, store) = fixture();
        fs::write(store.config_path(), b"model='fixture'\n").unwrap();
        let applied = apply(&store, "test");
        let current = store.current().unwrap();
        let (noop, changes) = config::patch(
            current.as_deref(),
            &[Edit {
                key: KEYS[0].into(),
                value: Some("test".into()),
            }],
        )
        .unwrap();
        assert!(changes.is_empty());
        assert!(
            !store
                .commit(&applied.revision, Some(&noop), vec![], "apply")
                .unwrap()
                .changed
        );
        let mut drifted = noop;
        drifted.extend_from_slice(b"\n[projects.'D:/new']\ntrust_level='trusted'\n");
        fs::write(store.config_path(), &drifted).unwrap();
        let (restored, keys) = store
            .restore_bytes(applied.snapshot_id.as_ref().unwrap(), Some(&drifted))
            .unwrap();
        store
            .commit(
                &config::revision(Some(&drifted)),
                restored.as_deref(),
                keys,
                "restore",
            )
            .unwrap();
        let text = String::from_utf8(store.current().unwrap().unwrap()).unwrap();
        assert!(text.contains("D:/new"));
        assert!(!text.contains(KEYS[0]));
    }
    #[test]
    fn stale_preview_and_owned_conflict_never_overwrite() {
        let (_dir, store) = fixture();
        let applied = apply(&store, "test");
        let modified = b"developer_instructions='user'\n";
        fs::write(store.config_path(), modified).unwrap();
        assert_eq!(
            store
                .commit(&applied.revision, Some(b""), vec![KEYS[0].into()], "apply")
                .unwrap_err()
                .code,
            "DESKTOP_STALE"
        );
        assert!(store
            .restore_bytes(applied.snapshot_id.as_ref().unwrap(), Some(modified))
            .is_err());
        assert_eq!(store.current().unwrap().unwrap(), modified);
    }
    #[test]
    fn interrupted_commit_has_a_recoverable_verified_snapshot() {
        let (_dir, mut store) = fixture();
        store.fail_after_replace = true;
        let (next, _) = config::patch(
            None,
            &[Edit {
                key: KEYS[0].into(),
                value: Some("test".into()),
            }],
        )
        .unwrap();
        assert!(store
            .commit("missing", Some(&next), vec![KEYS[0].into()], "apply")
            .is_err());
        let history = store.history(Some(&next)).unwrap();
        assert_eq!(history[0].state, "prepared");
        assert!(history[0].can_restore);
        assert!(store
            .restore_bytes(&history[0].id, Some(&next))
            .unwrap()
            .0
            .is_none());
    }
    #[test]
    fn tampered_backup_and_path_traversal_are_rejected() {
        let (_dir, store) = fixture();
        let receipt = apply(&store, "test");
        let id = receipt.snapshot_id.unwrap();
        fs::write(store.folder(&id).unwrap().join("after.toml"), b"corrupt").unwrap();
        assert_eq!(
            store
                .restore_bytes(&id, store.current().unwrap().as_deref())
                .unwrap_err()
                .code,
            "DESKTOP_SNAPSHOT"
        );
        assert!(store.folder("../outside").is_err());
    }
    #[test]
    fn first_apply_restore_removes_only_generated_config() {
        let (_dir, store) = fixture();
        let applied = apply(&store, "test");
        let current = store.current().unwrap();
        let (restored, keys) = store
            .restore_bytes(&applied.snapshot_id.unwrap(), current.as_deref())
            .unwrap();
        store
            .commit(&applied.revision, restored.as_deref(), keys, "restore")
            .unwrap();
        assert!(store.current().unwrap().is_none());
        assert_eq!(store.history(None).unwrap().len(), 2);
    }
    #[test]
    fn malformed_history_is_visible_not_silently_healthy() {
        let (_dir, store) = fixture();
        fs::create_dir_all(store.root.join("123-1-1")).unwrap();
        fs::write(store.root.join("123-1-1/entry.json"), b"invalid").unwrap();
        let history = store.history(None).unwrap();
        assert_eq!(history[0].state, "invalid");
        assert!(!history[0].can_restore);
    }
    #[test]
    fn linked_config_is_rejected() {
        let (dir, store) = fixture();
        let outside = dir.path().join("outside");
        fs::write(&outside, b"untouched").unwrap();
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(&outside, store.config_path());
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(&outside, store.config_path());
        if result.is_ok() {
            assert_eq!(store.current().unwrap_err().code, "DESKTOP_LINK");
        }
        assert_eq!(fs::read(outside).unwrap(), b"untouched");
    }
}
