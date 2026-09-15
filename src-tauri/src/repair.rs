use crate::{
    error::AppError,
    model::{CodexStatus, Health},
    windows,
};
use serde::Serialize;
use std::sync::Mutex;
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    RegKey,
};

static REPAIR_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize)]
pub struct RepairResult {
    pub status: CodexStatus,
    pub changed: bool,
    pub previous_path: Option<String>,
    pub notification_sent: bool,
}

fn validate(status: &CodexStatus, expected: &str, previous: Option<&str>) -> Result<(), AppError> {
    if !matches!(status.health, Health::Healthy | Health::RepairRecommended)
        || !status.installed
        || status.expected_cli.exists != Some(true)
        || status.expected_cli.is_file != Some(true)
        || status.expected_cli.accessible != Some(true)
        || status
            .expected_cli_path
            .as_deref()
            .is_none_or(str::is_empty)
        || status
            .issues
            .iter()
            .any(|e| e.code == "ENVIRONMENT_READ_FAILED")
    {
        return Err(AppError::new(
            "REPAIR_UNAVAILABLE",
            "Cannot safely repair CLI path",
            "请先确认 Codex 安装完整、内置 CLI 可读且用户环境变量可读取。",
        ));
    }
    if status.expected_cli_path.as_deref() != Some(expected)
        || status.current_cli_path.as_deref() != previous
    {
        return Err(AppError::new(
            "REPAIR_STALE",
            "Repair preconditions changed",
            "安装包或环境变量已变化，请刷新后重新确认。",
        ));
    }
    Ok(())
}

fn write_path(key: &RegKey, target: &str) -> Result<(), AppError> {
    key.set_value("CODEX_CLI_PATH", &target).map_err(|e| {
        AppError::new(
            "ENVIRONMENT_WRITE_FAILED",
            "Cannot save CODEX_CLI_PATH",
            e.to_string(),
        )
    })?;
    let actual: String = key.get_value("CODEX_CLI_PATH").map_err(|e| {
        AppError::new(
            "REPAIR_VERIFY_FAILED",
            "Path saved but readback failed",
            e.to_string(),
        )
    })?;
    if actual != target {
        return Err(AppError::new(
            "REPAIR_VERIFY_FAILED",
            "Path changed after writing",
            "写入后读回的值不一致，请重新检测。",
        ));
    }
    Ok(())
}

fn notify_environment() -> bool {
    use ::windows::{
        core::w,
        Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::WindowsAndMessaging::{
                SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
            },
        },
    };
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(w!("Environment").as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        )
        .0 != 0
    }
}

pub fn repair(
    confirmed: bool,
    expected: &str,
    previous: Option<&str>,
) -> Result<RepairResult, AppError> {
    if !confirmed {
        return Err(AppError::new(
            "CONFIRMATION_REQUIRED",
            "Confirm path repair first",
            "尚未写入任何环境变量。",
        ));
    }
    let _lock = REPAIR_LOCK
        .try_lock()
        .map_err(|_| AppError::new("OPERATION_BUSY", "Repair already running", "请稍后重试。"))?;
    let before = windows::detect()?;
    validate(&before, expected, previous)?;
    if before.path_matches {
        return Ok(RepairResult {
            status: before,
            changed: false,
            previous_path: previous.map(str::to_owned),
            notification_sent: true,
        });
    }
    // The write target comes exclusively from a fresh AppX detection.
    let target = before.expected_cli_path.as_deref().unwrap();
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| {
            AppError::new(
                "ENVIRONMENT_WRITE_FAILED",
                "Cannot open user environment for writing",
                e.to_string(),
            )
        })?;
    if windows::current_path()?.as_deref() != previous {
        return Err(AppError::new(
            "REPAIR_STALE",
            "Environment changed",
            "环境变量已变化，请刷新后重新确认。",
        ));
    }
    write_path(&key, target)?;
    let notification_sent = notify_environment();
    let status = windows::detect().map_err(|e| {
        AppError::new(
            "REPAIR_VERIFY_FAILED",
            "Path saved but detection failed",
            e.to_string(),
        )
    })?;
    if status.health != Health::Healthy {
        return Err(AppError::new(
            "REPAIR_VERIFY_FAILED",
            "Path saved but health check failed",
            "环境变量已写入，但安装包或环境变量可能再次发生变化，请重新检测。",
        ));
    }
    Ok(RepairResult {
        status,
        changed: true,
        previous_path: previous.map(str::to_owned),
        notification_sent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FileStatus;
    fn status() -> CodexStatus {
        CodexStatus {
            installed: true,
            version: None,
            install_location: None,
            package: None,
            expected_cli_path: Some("C:\\Codex\\codex.exe".into()),
            expected_cli_exists: Some(true),
            current_cli_path: None,
            current_cli_exists: None,
            path_matches: false,
            expected_cli: FileStatus {
                exists: Some(true),
                is_file: Some(true),
                accessible: Some(true),
                ..Default::default()
            },
            current_cli: FileStatus::default(),
            health: Health::RepairRecommended,
            checked_at_unix_ms: 0,
            issues: vec![],
        }
    }
    #[test]
    fn confirmation_is_required_before_detection_or_write() {
        assert_eq!(
            repair(false, "", None).unwrap_err().code,
            "CONFIRMATION_REQUIRED"
        );
    }
    #[test]
    fn rejects_stale_target_and_changed_previous_value() {
        let s = status();
        assert!(validate(&s, s.expected_cli_path.as_deref().unwrap(), None).is_ok());
        assert_eq!(
            validate(&s, "C:\\arbitrary.exe", None).unwrap_err().code,
            "REPAIR_STALE"
        );
        assert_eq!(
            validate(&s, s.expected_cli_path.as_deref().unwrap(), Some("old"))
                .unwrap_err()
                .code,
            "REPAIR_STALE"
        );
    }
    #[test]
    fn refuses_missing_unreadable_and_unknown_installations() {
        for health in [
            Health::NotInstalled,
            Health::InstallationIncomplete,
            Health::DetectionError,
        ] {
            let mut s = status();
            s.health = health;
            assert_eq!(
                validate(&s, s.expected_cli_path.as_deref().unwrap(), None)
                    .unwrap_err()
                    .code,
                "REPAIR_UNAVAILABLE"
            );
        }
        let mut s = status();
        s.expected_cli.accessible = Some(false);
        assert!(validate(&s, s.expected_cli_path.as_deref().unwrap(), None).is_err());
    }
    #[test]
    fn registry_roundtrip_preserves_other_values_and_reports_access_denied() {
        let name = format!(
            "Software\\CodexGuardTests\\repair-{}-{}",
            std::process::id(),
            crate::proxy::now()
        );
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey(&name).unwrap();
        key.set_value("UNRELATED", &"unchanged").unwrap();
        write_path(&key, "C:\\新版本\\codex.exe").unwrap();
        write_path(&key, "C:\\next\\codex.exe").unwrap();
        assert_eq!(
            key.get_value::<String, _>("CODEX_CLI_PATH").unwrap(),
            "C:\\next\\codex.exe"
        );
        assert_eq!(
            key.get_value::<String, _>("UNRELATED").unwrap(),
            "unchanged"
        );
        let readonly = root.open_subkey_with_flags(&name, KEY_READ).unwrap();
        assert_eq!(
            write_path(&readonly, "denied").unwrap_err().code,
            "ENVIRONMENT_WRITE_FAILED"
        );
        drop(readonly);
        drop(key);
        root.delete_subkey_all(&name).unwrap();
    }
}
