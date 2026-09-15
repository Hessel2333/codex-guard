use crate::{
    error::AppError,
    model::{evaluate_health, AppPackage, CodexStatus, FileStatus},
};
use std::{
    fs::{self, File},
    io::ErrorKind,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use windows::{
    core::{w, HSTRING},
    Management::Deployment::{PackageManager, PackageTypes},
    Win32::{
        Globalization::{CompareStringOrdinal, CSTR_EQUAL},
        Storage::FileSystem::{
            GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
        },
        System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    },
};
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ},
    RegKey,
};

// Balances every successful RoInitialize on the same blocking worker thread.
struct Apartment;
impl Apartment {
    fn new() -> Result<Self, AppError> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(|e| api_error("Initialize Windows Runtime", e))?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

fn api_error(operation: &str, error: windows::core::Error) -> AppError {
    AppError::new(
        "WINDOWS_API_ERROR",
        operation,
        format!(
            "{}; HRESULT 0x{:08X}",
            error.message(),
            error.code().0 as u32
        ),
    )
}

pub(crate) fn package() -> Result<Option<AppPackage>, AppError> {
    let _apartment = Apartment::new()?;
    let manager = PackageManager::new().map_err(|e| api_error("Create PackageManager", e))?;
    // Empty SID means current user. Never enumerate all users or request elevation.
    let packages = manager
        .FindPackagesByUserSecurityIdWithPackageTypes(&HSTRING::new(), PackageTypes::Main)
        .map_err(|e| api_error("Read current user's AppX packages", e))?;
    let iterator = packages
        .First()
        .map_err(|e| api_error("Enumerate AppX packages", e))?;
    let mut newest: Option<([u16; 4], windows::ApplicationModel::Package)> = None;
    // Explicit iterator preserves COM errors that an infallible IntoIterator can hide.
    while iterator
        .HasCurrent()
        .map_err(|e| api_error("Read AppX iterator", e))?
    {
        let pkg = iterator
            .Current()
            .map_err(|e| api_error("Read AppX package", e))?;
        let id = pkg.Id().map_err(|e| api_error("Read AppX identity", e))?;
        if id
            .Name()
            .map_err(|e| api_error("Read package name", e))?
            .to_string()
            == "OpenAI.Codex"
        {
            let v = id
                .Version()
                .map_err(|e| api_error("Read Codex version", e))?;
            let version = [v.Major, v.Minor, v.Build, v.Revision];
            if newest
                .as_ref()
                .is_none_or(|(previous, _)| version > *previous)
            {
                newest = Some((version, pkg));
            }
        }
        iterator
            .MoveNext()
            .map_err(|e| api_error("Advance AppX iterator", e))?;
    }
    newest
        .map(|(v, pkg)| {
            let id = pkg.Id().map_err(|e| api_error("Read Codex identity", e))?;
            let location = pkg
                .InstalledLocation()
                .and_then(|folder| folder.Path())
                .map_err(|e| api_error("Read Codex install location", e))?
                .to_string();
            if location.is_empty() || !Path::new(&location).is_absolute() {
                return Err(AppError::new(
                    "INVALID_INSTALL_LOCATION",
                    "AppX returned an invalid install location",
                    location,
                ));
            }
            Ok(AppPackage {
                name: "OpenAI.Codex".into(),
                version: format!("{}.{}.{}.{}", v[0], v[1], v[2], v[3]),
                install_location: location,
                package_full_name: id
                    .FullName()
                    .map_err(|e| api_error("Read package full name", e))?
                    .to_string(),
                package_family_name: id
                    .FamilyName()
                    .map_err(|e| api_error("Read package family", e))?
                    .to_string(),
            })
        })
        .transpose()
}

fn current_path() -> Result<Option<String>, AppError> {
    let env =
        match RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Environment", KEY_READ) {
            Ok(key) => key,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(AppError::new(
                    "ENVIRONMENT_READ_FAILED",
                    "Cannot read current user's environment",
                    e.to_string(),
                ))
            }
        };
    match env.get_value::<String, _>("CODEX_CLI_PATH") {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AppError::new(
            "ENVIRONMENT_READ_FAILED",
            "Cannot read CODEX_CLI_PATH",
            e.to_string(),
        )),
    }
}

fn file_version(path: &str) -> Option<String> {
    let path = HSTRING::from(path);
    unsafe {
        let size = GetFileVersionInfoSizeW(&path, None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(&path, None, size, data.as_mut_ptr().cast()).ok()?;
        let mut info = std::ptr::null_mut();
        let mut length = 0;
        if !VerQueryValueW(data.as_ptr().cast(), w!("\\"), &mut info, &mut length).as_bool()
            || info.is_null()
            || length < std::mem::size_of::<VS_FIXEDFILEINFO>() as u32
        {
            return None;
        }
        // Resource buffer is byte aligned, so read_unaligned rather than dereferencing.
        let info = std::ptr::read_unaligned(info.cast::<VS_FIXEDFILEINFO>());
        if info.dwSignature != 0xFEEF04BD {
            return None;
        }
        Some(format!(
            "{}.{}.{}.{}",
            info.dwFileVersionMS >> 16,
            info.dwFileVersionMS & 0xffff,
            info.dwFileVersionLS >> 16,
            info.dwFileVersionLS & 0xffff
        ))
    }
}

fn inspect_file(path: Option<&str>) -> FileStatus {
    let mut status = FileStatus {
        path: path.map(str::to_owned),
        ..Default::default()
    };
    let Some(path) = path.filter(|p| !p.is_empty()) else {
        return status;
    };
    if !Path::new(path).is_absolute() {
        status.error = Some(AppError::new(
            "INVALID_CLI_PATH",
            "CLI path must be an absolute Windows path",
            path,
        ));
        return status;
    }
    match fs::metadata(path) {
        Ok(metadata) => {
            status.exists = Some(true);
            status.is_file = Some(metadata.is_file());
            status.size = Some(metadata.len());
            status.modified_unix_ms = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|t| t.as_millis() as u64);
            if metadata.is_file() {
                match File::open(path) {
                    Ok(_) => {
                        status.accessible = Some(true);
                        status.file_version = file_version(path);
                    }
                    Err(e) => {
                        status.accessible = Some(false);
                        status.error = Some(AppError::new(
                            "CLI_ACCESS_FAILED",
                            "Cannot open CLI for reading",
                            format!("{path}: {e}"),
                        ));
                    }
                }
            } else {
                status.accessible = Some(false);
                status.error = Some(AppError::new(
                    "CLI_NOT_A_FILE",
                    "CLI path points to a directory",
                    path,
                ));
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            status.exists = Some(false);
            status.is_file = Some(false);
            status.accessible = Some(false);
        }
        Err(e) => {
            status.error = Some(AppError::new(
                "CLI_METADATA_FAILED",
                "Cannot inspect CLI file",
                format!("{path}: {e}"),
            ))
        }
    }
    status
}

fn paths_match(current: Option<&str>, expected: Option<&str>) -> bool {
    match (current, expected) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => {
            let a: Vec<u16> = a.replace('/', "\\").encode_utf16().collect();
            let b: Vec<u16> = b.replace('/', "\\").encode_utf16().collect();
            unsafe { CompareStringOrdinal(&a, &b, true) == CSTR_EQUAL }
        }
        _ => false,
    }
}

pub fn detect() -> Result<CodexStatus, AppError> {
    let package = package()?;
    let expected_path = package.as_ref().map(|pkg| {
        Path::new(&pkg.install_location)
            .join("app")
            .join("resources")
            .join("codex.exe")
            .to_string_lossy()
            .into_owned()
    });
    let mut issues = Vec::new();
    let current_path = match current_path() {
        Ok(path) => path,
        Err(e) => {
            issues.push(e);
            None
        }
    };
    let environment_read_failed = !issues.is_empty();
    let expected_cli = inspect_file(expected_path.as_deref());
    let current_cli = if paths_match(current_path.as_deref(), expected_path.as_deref()) {
        FileStatus {
            path: current_path.clone(),
            ..expected_cli.clone()
        }
    } else {
        inspect_file(current_path.as_deref())
    };
    issues.extend(expected_cli.error.iter().cloned());
    issues.extend(current_cli.error.iter().cloned());
    let path_matches = paths_match(current_path.as_deref(), expected_path.as_deref());
    Ok(CodexStatus {
        installed: package.is_some(),
        version: package.as_ref().map(|pkg| pkg.version.clone()),
        install_location: package.as_ref().map(|pkg| pkg.install_location.clone()),
        health: evaluate_health(
            package.is_some(),
            &expected_cli,
            &current_cli,
            path_matches,
            environment_read_failed,
        ),
        package,
        expected_cli_path: expected_path,
        expected_cli_exists: expected_cli.exists,
        current_cli_path: current_path,
        current_cli_exists: current_cli.exists,
        path_matches,
        expected_cli,
        current_cli,
        checked_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_path_case_and_separators_match() {
        assert!(paths_match(
            Some("C:/WindowsApps/CODEX.exe"),
            Some("c:\\windowsapps\\codex.exe")
        ));
        assert!(!paths_match(
            Some("\"C:\\codex.exe\""),
            Some("C:\\codex.exe")
        ));
        assert!(!paths_match(Some(""), Some("")));
    }
    #[test]
    fn relative_path_is_invalid_not_resolved_against_guard_folder() {
        let status = inspect_file(Some("codex.exe"));
        assert!(status.exists.is_none());
        assert_eq!(status.error.unwrap().code, "INVALID_CLI_PATH");
    }
}
