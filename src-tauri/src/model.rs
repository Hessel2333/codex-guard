use crate::error::AppError;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppPackage {
    pub name: String,
    pub version: String,
    pub install_location: String,
    pub package_full_name: String,
    pub package_family_name: String,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct FileStatus {
    pub path: Option<String>,
    // None means unknown, e.g. access denied. Never report it as missing.
    pub exists: Option<bool>,
    pub is_file: Option<bool>,
    pub accessible: Option<bool>,
    pub size: Option<u64>,
    pub modified_unix_ms: Option<u64>,
    pub file_version: Option<String>,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Healthy,
    RepairRecommended,
    NotInstalled,
    InstallationIncomplete,
    DetectionError,
}

#[derive(Debug, Serialize)]
pub struct CodexStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub install_location: Option<String>,
    pub package: Option<AppPackage>,
    pub expected_cli_path: Option<String>,
    pub expected_cli_exists: Option<bool>,
    pub current_cli_path: Option<String>,
    pub current_cli_exists: Option<bool>,
    pub path_matches: bool,
    pub expected_cli: FileStatus,
    pub current_cli: FileStatus,
    pub health: Health,
    pub checked_at_unix_ms: u64,
    pub issues: Vec<AppError>,
}

pub fn evaluate_health(
    installed: bool,
    expected: &FileStatus,
    current: &FileStatus,
    path_matches: bool,
    environment_read_failed: bool,
) -> Health {
    if !installed {
        return Health::NotInstalled;
    }
    if expected.exists == Some(false) || expected.is_file == Some(false) {
        return Health::InstallationIncomplete;
    }
    if environment_read_failed || expected.exists.is_none() || expected.accessible != Some(true) {
        return Health::DetectionError;
    }
    if path_matches
        && current.exists == Some(true)
        && current.is_file == Some(true)
        && current.accessible == Some(true)
    {
        Health::Healthy
    } else if path_matches && current.error.is_some() {
        Health::DetectionError
    } else {
        Health::RepairRecommended
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(exists: bool) -> FileStatus {
        FileStatus {
            exists: Some(exists),
            is_file: Some(exists),
            accessible: Some(exists),
            ..Default::default()
        }
    }
    #[test]
    fn scenario_a_current_bundled_file_is_healthy() {
        assert_eq!(
            evaluate_health(true, &file(true), &file(true), true, false),
            Health::Healthy
        );
    }
    #[test]
    fn scenario_b_stale_deleted_path_needs_repair() {
        assert_eq!(
            evaluate_health(true, &file(true), &file(false), false, false),
            Health::RepairRecommended
        );
    }
    #[test]
    fn scenario_d_absent_package_is_not_a_crash() {
        assert_eq!(
            evaluate_health(false, &FileStatus::default(), &file(false), false, false),
            Health::NotInstalled
        );
    }
    #[test]
    fn scenario_e_missing_bundled_file_is_incomplete() {
        assert_eq!(
            evaluate_health(true, &file(false), &file(true), false, false),
            Health::InstallationIncomplete
        );
    }
    #[test]
    fn unreadable_expected_path_is_unknown_not_missing() {
        assert_eq!(
            evaluate_health(true, &FileStatus::default(), &file(true), false, false),
            Health::DetectionError
        );
    }
    #[test]
    fn unset_environment_needs_repair() {
        assert_eq!(
            evaluate_health(true, &file(true), &FileStatus::default(), false, false),
            Health::RepairRecommended
        );
    }
    #[test]
    fn registry_access_denied_cannot_be_healthy() {
        assert_eq!(
            evaluate_health(true, &file(true), &file(true), true, true),
            Health::DetectionError
        );
    }
    #[test]
    fn directory_named_codex_exe_is_not_valid() {
        let mut expected = file(true);
        expected.is_file = Some(false);
        assert_eq!(
            evaluate_health(true, &expected, &file(true), true, false),
            Health::InstallationIncomplete
        );
    }
    #[test]
    fn existing_wrong_version_still_requires_repair() {
        assert_eq!(
            evaluate_health(true, &file(true), &file(true), false, false),
            Health::RepairRecommended
        );
    }
}
