//! Desktop-first configuration management. This does not patch the distributed
//! application, take over CLI shims, or make claims about cloud/session policy.
pub mod config;
mod runtime;
mod storage;

use crate::{error::AppError, proxy};
use config::{err, Change, Edit, Setting};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};
use storage::{History, Receipt, Store};

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub home: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub executable: String,
    pub cli_path: String,
    pub desktop_version: Option<String>,
    pub identity: String,
    pub source: String,
    pub running_pids: Vec<u32>,
    pub unverified_processes: usize,
}
#[derive(Debug, Serialize)]
pub struct Status {
    pub home: String,
    pub home_source: String,
    pub home_runtime_verified: bool,
    pub config_path: String,
    pub config_exists: bool,
    pub revision: Option<String>,
    pub settings: Vec<Setting>,
    pub config_error: Option<AppError>,
    pub external_manager: bool,
    pub target: Option<Target>,
    pub target_error: Option<AppError>,
    pub history: Vec<History>,
    pub history_error: Option<AppError>,
    pub backup_path: String,
    pub checked_at: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub home: String,
    pub revision: String,
    pub target_identity: String,
    pub after_revision: String,
    pub changes: Vec<Change>,
    pub operation: String,
    pub snapshot_id: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    pub selection: Selection,
    pub revision: String,
    pub target_identity: String,
    pub edits: Vec<Edit>,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchRequest {
    pub selection: Selection,
    pub revision: String,
    pub target_identity: String,
    pub use_proxy: bool,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreRequest {
    pub selection: Selection,
    pub revision: String,
    pub target_identity: String,
    pub snapshot_id: String,
    pub confirmed: bool,
}
#[derive(Debug, Serialize)]
pub struct Probe {
    pub home: String,
    pub revision: String,
    pub target_identity: String,
    pub evidence: runtime::Evidence,
}
#[derive(Debug, Serialize)]
pub struct OperationResult {
    pub receipt: Receipt,
    pub verification: Probe,
}

pub fn normalize_home(input: &Path) -> Result<PathBuf, AppError> {
    if !input.is_absolute()
        || input.parent().is_none()
        || input
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || input.to_string_lossy().chars().any(char::is_control)
    {
        return Err(err(
            "DESKTOP_HOME",
            "配置目录必须是完整的真实路径",
            "不要使用引号、相对路径或 ..。",
        ));
    }
    config::plain_path(input)?;
    let mut parent = input;
    let mut tail = Vec::new();
    loop {
        match fs::metadata(parent) {
            Ok(meta) => {
                if !meta.is_dir() {
                    return Err(err("DESKTOP_HOME", "配置目录不是文件夹", parent.display()));
                }
                let mut canonical = fs::canonicalize(parent)
                    .map_err(|e| err("DESKTOP_HOME", "无法确认配置目录", e))?;
                for name in tail.into_iter().rev() {
                    canonical.push(name);
                }
                return Ok(canonical);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tail.push(
                    parent
                        .file_name()
                        .ok_or_else(|| err("DESKTOP_HOME", "配置目录无效", input.display()))?
                        .to_os_string(),
                );
                parent = parent
                    .parent()
                    .ok_or_else(|| err("DESKTOP_HOME", "配置目录无效", input.display()))?;
            }
            Err(e) => return Err(err("DESKTOP_HOME", "无法访问配置目录", e)),
        }
    }
}
fn home(selection: &Selection) -> Result<(PathBuf, &'static str), AppError> {
    if let Some(value) = &selection.home {
        if value.trim().is_empty() {
            return Err(err(
                "DESKTOP_HOME",
                "配置目录不能为空",
                "使用自动选择或填写完整路径。",
            ));
        }
        return Ok((normalize_home(Path::new(value))?, "selected"));
    }
    if let Some(value) = std::env::var_os("CODEX_HOME").filter(|s| !s.is_empty()) {
        return Ok((normalize_home(Path::new(&value))?, "environment"));
    }
    let profile = std::env::var_os("USERPROFILE").ok_or_else(|| {
        err(
            "DESKTOP_HOME",
            "无法确认当前用户目录",
            "请手动选择 Codex 配置目录。",
        )
    })?;
    Ok((
        normalize_home(&PathBuf::from(profile).join(".codex"))?,
        "default",
    ))
}
fn context(selection: &Selection) -> Result<(Store, &'static str), AppError> {
    let (home, source) = home(selection)?;
    Ok((Store::new(home, &proxy::data_dir()?), source))
}
fn target() -> Result<Target, AppError> {
    let state = proxy::desktop_instance()?;
    let exe = fs::canonicalize(&state.executable)
        .map_err(|e| err("DESKTOP_IDENTITY", "无法确认桌面程序路径", e))?;
    let cli = exe
        .parent()
        .ok_or_else(|| err("DESKTOP_IDENTITY", "桌面程序路径无效", exe.display()))?
        .join("resources/codex.exe");
    config::plain_path(&cli)?;
    let meta = fs::metadata(&cli).map_err(|e| err("DESKTOP_CLI", "内置 CLI 不可用", e))?;
    if !meta.is_file() {
        return Err(err("DESKTOP_CLI", "内置 CLI 不是普通文件", cli.display()));
    }
    fs::File::open(&cli).map_err(|e| err("DESKTOP_CLI", "无法读取内置 CLI", e))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
        .map(|v| v.as_nanos())
        .unwrap_or_default();
    let desktop_meta =
        fs::metadata(&exe).map_err(|e| err("DESKTOP_IDENTITY", "无法检查桌面程序", e))?;
    let desktop_modified = desktop_meta
        .modified()
        .ok()
        .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
        .map(|v| v.as_nanos())
        .unwrap_or_default();
    let identity = config::digest(
        format!(
            "{}|{}|{}|{modified}|{}|{desktop_modified}",
            exe.to_string_lossy().to_lowercase(),
            cli.to_string_lossy().to_lowercase(),
            meta.len(),
            desktop_meta.len()
        )
        .as_bytes(),
    );
    Ok(Target {
        executable: exe.to_string_lossy().into_owned(),
        cli_path: cli.to_string_lossy().into_owned(),
        desktop_version: crate::windows::package()
            .ok()
            .flatten()
            .and_then(|package| {
                let root = fs::canonicalize(&package.install_location).ok()?;
                if exe.starts_with(root) {
                    Some(package.version)
                } else {
                    None
                }
            }),
        identity,
        source: if state.manual { "manual" } else { "discovered" }.into(),
        running_pids: state
            .processes
            .iter()
            .filter(|p| p.managed)
            .map(|p| p.pid)
            .collect(),
        unverified_processes: state.processes.iter().filter(|p| p.path.is_none()).count(),
    })
}
fn external(bytes: Option<&[u8]>) -> bool {
    bytes.is_some_and(|b| String::from_utf8_lossy(b).contains("# >>> codex-purge managed block"))
}
fn editable(bytes: Option<&[u8]>) -> Result<(), AppError> {
    if external(bytes) {
        return Err(err(
            "DESKTOP_EXTERNAL_MANAGER",
            "配置正由其他工具管理",
            "请先在原工具中恢复受管配置，再由 Guard 管理，避免相互覆盖。",
        ));
    }
    config::parse(bytes)?;
    Ok(())
}
pub fn status(selection: Selection) -> Result<Status, AppError> {
    let (store, source) = context(&selection)?;
    let (target, target_error) = match target() {
        Ok(t) => (Some(t), None),
        Err(e) => (None, Some(e)),
    };
    let read = store.current();
    let (bytes, mut config_error) = match read {
        Ok(b) => (b, None),
        Err(e) => (None, Some(e)),
    };
    let parsed = config::parse(bytes.as_deref());
    let settings = match parsed {
        Ok(doc) if config_error.is_none() => config::settings(&doc),
        Ok(_) => vec![],
        Err(e) => {
            config_error = Some(e);
            vec![]
        }
    };
    let (history, history_error) = if config_error.is_none() {
        match store.history(bytes.as_deref()) {
            Ok(h) => (h, None),
            Err(e) => (vec![], Some(e)),
        }
    } else {
        (vec![], None)
    };
    Ok(Status {
        home: store.home.to_string_lossy().into_owned(),
        home_source: source.into(),
        home_runtime_verified: false,
        config_path: store.config_path().to_string_lossy().into_owned(),
        config_exists: bytes.is_some(),
        revision: if config_error.is_none() {
            Some(config::revision(bytes.as_deref()))
        } else {
            None
        },
        settings,
        config_error,
        external_manager: external(bytes.as_deref()),
        target,
        target_error,
        history,
        history_error,
        backup_path: store.root.to_string_lossy().into_owned(),
        checked_at: storage::now(),
    })
}
fn check_target(expected: &str) -> Result<Target, AppError> {
    let target = target()?;
    if target.identity != expected {
        return Err(err(
            "DESKTOP_TARGET_STALE",
            "桌面安装已变化，请刷新后重新预览",
            "未写入旧版本或其他安装的配置。",
        ));
    }
    Ok(target)
}
fn check_revision(store: &Store, revision: &str) -> Result<Option<Vec<u8>>, AppError> {
    let bytes = store.current()?;
    if config::revision(bytes.as_deref()) != revision {
        return Err(err(
            "DESKTOP_STALE",
            "配置已发生变化，请重新预览",
            "未覆盖其他操作的修改。",
        ));
    }
    editable(bytes.as_deref())?;
    Ok(bytes)
}
pub fn preview(
    selection: Selection,
    revision: String,
    target_identity: String,
    edits: Vec<Edit>,
) -> Result<Preview, AppError> {
    let (store, _) = context(&selection)?;
    let _target = check_target(&target_identity)?;
    let before = check_revision(&store, &revision)?;
    let (next, changes) = config::patch(before.as_deref(), &edits)?;
    Ok(Preview {
        home: store.home.to_string_lossy().into_owned(),
        revision,
        target_identity,
        after_revision: config::revision(Some(&next)),
        changes,
        operation: "apply".into(),
        snapshot_id: None,
    })
}
pub fn preview_restore(
    selection: Selection,
    revision: String,
    target_identity: String,
    snapshot_id: String,
) -> Result<Preview, AppError> {
    let (store, _) = context(&selection)?;
    check_target(&target_identity)?;
    let before = check_revision(&store, &revision)?;
    let (next, keys) = store.restore_bytes(&snapshot_id, before.as_deref())?;
    let old = config::settings(&config::parse(before.as_deref())?);
    let new = config::settings(&config::parse(next.as_deref())?);
    let changes = keys
        .iter()
        .map(|key| Change {
            key: key.clone(),
            before: old.iter().find(|s| &s.key == key).and_then(|s| {
                if s.advanced {
                    Some("高级配置".into())
                } else {
                    s.value.clone()
                }
            }),
            after: new.iter().find(|s| &s.key == key).and_then(|s| {
                if s.advanced {
                    Some("高级配置".into())
                } else {
                    s.value.clone()
                }
            }),
        })
        .collect();
    Ok(Preview {
        home: store.home.to_string_lossy().into_owned(),
        revision,
        target_identity,
        after_revision: config::revision(next.as_deref()),
        changes,
        operation: "restore".into(),
        snapshot_id: Some(snapshot_id),
    })
}
fn confirmed(value: bool) -> Result<(), AppError> {
    if value {
        Ok(())
    } else {
        Err(err(
            "CONFIRMATION_REQUIRED",
            "修改配置需要确认",
            "请先预览变更并确认目标目录。",
        ))
    }
}
pub fn launch(request: LaunchRequest) -> Result<proxy::LaunchReceipt, AppError> {
    if !request.confirmed {
        return Err(err(
            "CONFIRMATION_REQUIRED",
            "启动 Codex 需要确认",
            "请先核对配置目录与代理选项。",
        ));
    }
    let _lock = proxy::lock("desktop-config.lock")?;
    let (store, _) = context(&request.selection)?;
    let target = check_target(&request.target_identity)?;
    // Launch consumes existing configuration, including externally managed
    // instructions, without taking ownership or rewriting any configuration.
    proxy::launch_with_environment(
        true,
        Some(&store.home),
        request.use_proxy,
        Some(Path::new(&target.executable)),
        || {
            check_target(&request.target_identity)?;
            let bytes = store.current()?;
            if config::revision(bytes.as_deref()) != request.revision {
                return Err(err(
                    "DESKTOP_STALE",
                    "配置已变化，请刷新后重新确认启动",
                    "未关闭任何现有进程。",
                ));
            }
            config::parse(bytes.as_deref())?;
            Ok(())
        },
    )
}

pub fn apply(request: ApplyRequest) -> Result<OperationResult, AppError> {
    confirmed(request.confirmed)?;
    let _lock = proxy::lock("desktop-config.lock")?;
    let (store, _) = context(&request.selection)?;
    let target = check_target(&request.target_identity)?;
    let before = check_revision(&store, &request.revision)?;
    let (next, changes) = config::patch(before.as_deref(), &request.edits)?;
    let evidence = runtime::verify(Path::new(&target.cli_path), Some(&next))?;
    check_target(&request.target_identity)?;
    let receipt = store.commit(
        &request.revision,
        Some(&next),
        changes.into_iter().map(|c| c.key).collect(),
        "apply",
    )?;
    let verification = Probe {
        home: store.home.to_string_lossy().into_owned(),
        revision: receipt.revision.clone(),
        target_identity: target.identity,
        evidence,
    };
    Ok(OperationResult {
        receipt,
        verification,
    })
}
pub fn restore(request: RestoreRequest) -> Result<OperationResult, AppError> {
    confirmed(request.confirmed)?;
    let _lock = proxy::lock("desktop-config.lock")?;
    let (store, _) = context(&request.selection)?;
    let target = check_target(&request.target_identity)?;
    let before = check_revision(&store, &request.revision)?;
    let (next, keys) = store.restore_bytes(&request.snapshot_id, before.as_deref())?;
    let evidence = runtime::verify(Path::new(&target.cli_path), next.as_deref())?;
    check_target(&request.target_identity)?;
    let receipt = store.commit(&request.revision, next.as_deref(), keys, "restore")?;
    let verification = Probe {
        home: store.home.to_string_lossy().into_owned(),
        revision: receipt.revision.clone(),
        target_identity: target.identity,
        evidence,
    };
    Ok(OperationResult {
        receipt,
        verification,
    })
}
pub fn verify(
    selection: Selection,
    revision: String,
    target_identity: String,
) -> Result<Probe, AppError> {
    let (store, _) = context(&selection)?;
    let target = check_target(&target_identity)?;
    let bytes = store.current()?;
    if config::revision(bytes.as_deref()) != revision {
        return Err(err(
            "DESKTOP_STALE",
            "配置已变化，请刷新后再检测",
            "未进行写入。",
        ));
    }
    let evidence = runtime::verify(Path::new(&target.cli_path), bytes.as_deref())?;
    check_target(&target_identity)?;
    if config::revision(store.current()?.as_deref()) != revision {
        return Err(err(
            "DESKTOP_STALE",
            "检测期间配置已变化",
            "本次结果已丢弃，请重新检测。",
        ));
    }
    Ok(Probe {
        home: store.home.to_string_lossy().into_owned(),
        revision,
        target_identity,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unconfirmed_operations_before_discovery_or_writes() {
        assert_eq!(
            launch(LaunchRequest {
                selection: Selection::default(),
                revision: "".into(),
                target_identity: "".into(),
                use_proxy: true,
                confirmed: false
            })
            .unwrap_err()
            .code,
            "CONFIRMATION_REQUIRED"
        );
        let request = ApplyRequest {
            selection: Selection::default(),
            revision: "missing".into(),
            target_identity: "".into(),
            edits: vec![],
            confirmed: false,
        };
        assert_eq!(apply(request).unwrap_err().code, "CONFIRMATION_REQUIRED");
        assert_eq!(
            restore(RestoreRequest {
                selection: Selection::default(),
                revision: "".into(),
                target_identity: "".into(),
                snapshot_id: "../bad".into(),
                confirmed: false
            })
            .unwrap_err()
            .code,
            "CONFIRMATION_REQUIRED"
        );
    }
    #[test]
    fn path_validation_does_not_create_selected_directories() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("new/nested");
        assert!(normalize_home(&home).is_ok());
        assert!(!home.exists());
        assert!(normalize_home(Path::new("relative/home")).is_err());
        assert!(normalize_home(&temp.path().join("../other")).is_err());
    }
    #[test]
    fn external_manager_is_not_silently_taken_over() {
        assert!(editable(Some(b"# >>> codex-purge managed block\nmodel='x'\n")).is_err());
    }
    #[test]
    #[ignore = "explicit desktop transaction integration: set GUARD_TEST_CODEX_CLI"]
    fn real_desktop_transaction_roundtrip() {
        let expected_cli =
            std::env::var_os("GUARD_TEST_CODEX_CLI").expect("GUARD_TEST_CODEX_CLI is required");
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("guard-data");
        proxy::TEST_STATE.with(|s| *s.borrow_mut() = Some(data));
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                proxy::TEST_STATE.with(|s| *s.borrow_mut() = None);
            }
        }
        let _reset = Reset;
        fs::create_dir_all(proxy::data_dir().unwrap()).unwrap();
        fs::write(
            proxy::data_dir().unwrap().join("proxy-last-launch.json"),
            b"broken unrelated history",
        )
        .unwrap();
        let home = temp.path().join("codex-home");
        fs::create_dir(&home).unwrap();
        let path = home.join("config.toml");
        let original = b"# user preferences\r\nmodel='fixture'\r\n";
        fs::write(&path, original).unwrap();
        let selection = Selection {
            home: Some(home.to_string_lossy().into_owned()),
        };
        let initial = status(selection.clone()).unwrap();
        let target = initial.target.unwrap();
        assert_eq!(
            fs::canonicalize(&target.cli_path).unwrap(),
            fs::canonicalize(expected_cli).unwrap()
        );
        assert!(!initial.home_runtime_verified);
        let edits = vec![
            Edit {
                key: "developer_instructions".into(),
                value: Some("desktop integration fixture".into()),
            },
            Edit {
                key: "model_reasoning_effort".into(),
                value: Some("low".into()),
            },
        ];
        let preview = preview(
            selection.clone(),
            initial.revision.unwrap(),
            target.identity.clone(),
            edits.clone(),
        )
        .unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            original,
            "preview must be read-only"
        );
        let applied = apply(ApplyRequest {
            selection: selection.clone(),
            revision: preview.revision,
            target_identity: target.identity.clone(),
            edits: edits.clone(),
            confirmed: true,
        })
        .unwrap();
        assert!(applied.receipt.changed);
        assert!(!applied.verification.evidence.active_session_verified);
        let noop = apply(ApplyRequest {
            selection: selection.clone(),
            revision: applied.receipt.revision.clone(),
            target_identity: target.identity.clone(),
            edits,
            confirmed: true,
        })
        .unwrap();
        assert!(!noop.receipt.changed);
        let mut drifted = fs::read(&path).unwrap();
        drifted.extend_from_slice(b"\n[projects.'D:/new']\ntrust_level='trusted'\n");
        fs::write(&path, &drifted).unwrap();
        let stale = restore(RestoreRequest {
            selection: selection.clone(),
            revision: applied.receipt.revision,
            target_identity: target.identity.clone(),
            snapshot_id: applied.receipt.snapshot_id.clone().unwrap(),
            confirmed: true,
        })
        .unwrap_err();
        assert_eq!(stale.code, "DESKTOP_STALE");
        let restored = restore(RestoreRequest {
            selection: selection.clone(),
            revision: config::revision(Some(&drifted)),
            target_identity: target.identity,
            snapshot_id: applied.receipt.snapshot_id.unwrap(),
            confirmed: true,
        })
        .unwrap();
        assert!(restored.receipt.changed);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("D:/new"));
        assert!(!text.contains("developer_instructions"));
        let final_status = status(selection).unwrap();
        assert_eq!(final_status.history.len(), 2);
        assert!(final_status
            .backup_path
            .starts_with(&temp.path().to_string_lossy().to_string()));
        println!("desktop transactions: preview -> apply -> no-op -> stale rejection -> drift-preserving restore; only temporary home/data used");
    }
}
