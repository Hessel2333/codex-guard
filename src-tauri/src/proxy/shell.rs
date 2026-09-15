use super::process::Handle;
use super::{data_dir, failure, lock, log, now, same_path, write_json, LaunchReceipt};
use crate::error::AppError;
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows::{
    core::{w, Interface, GUID, HSTRING, PCWSTR},
    Win32::{
        Foundation::{HANDLE, WAIT_OBJECT_0},
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
            },
            Threading::{
                GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, WaitForSingleObject,
            },
        },
        UI::{
            Shell::{
                FOLDERID_Desktop, FOLDERID_LocalAppData, IShellLinkW, SHGetKnownFolderPath,
                ShellExecuteExW, ShellLink, KF_FLAG_DEFAULT, SEE_MASK_NOASYNC,
                SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
            },
            WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL},
        },
    },
};

fn known_folder(id: &GUID) -> Result<PathBuf, AppError> {
    let value = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }
        .map_err(|e| failure("KNOWN_FOLDER", "Cannot locate Windows user folder", e))?;
    let path = unsafe { value.to_string() };
    unsafe {
        CoTaskMemFree(Some(value.0.cast()));
    }
    path.map(PathBuf::from)
        .map_err(|e| failure("KNOWN_FOLDER", "Invalid Windows user folder", e))
}
pub fn local_data() -> Result<PathBuf, AppError> {
    known_folder(&FOLDERID_LocalAppData)
}

pub fn is_elevated() -> Result<bool, AppError> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|e| failure("TOKEN_READ", "Cannot read process privileges", e))?;
        let token = Handle(token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut length = 0;
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        )
        .map_err(|e| failure("TOKEN_READ", "Cannot read administrator status", e))?;
        Ok(elevation.TokenIsElevated != 0)
    }
}

// Microsoft C runtime argv quoting; no cmd.exe / PowerShell command interpolation.
fn quote(argument: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for character in argument.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        if character == '"' {
            result.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
        } else {
            result.extend(std::iter::repeat_n('\\', slashes));
        }
        slashes = 0;
        result.push(character);
    }
    result.extend(std::iter::repeat_n('\\', slashes * 2));
    result.push('"');
    result
}

pub fn launch_elevated() -> Result<LaunchReceipt, AppError> {
    if is_elevated()? {
        return super::launch(true);
    }
    let _apartment = Apartment::new()?;
    let _dispatch = lock("proxy-elevation.lock")?;
    // Persist inherited CODEX_* overrides before elevation; UAC does not promise env inheritance.
    super::save_settings(super::get_settings()?)?;
    let folder = data_dir()?;
    let request = format!("{}-{}", std::process::id(), now());
    let file = HSTRING::from(
        std::env::current_exe()
            .map_err(|e| failure("SELF_PATH", "Cannot locate launcher executable", e))?
            .as_os_str(),
    );
    let parameters = HSTRING::from(format!(
        "--proxy-elevated {} {}",
        request,
        quote(&folder.to_string_lossy())
    ));
    log("Administrator proxy launch requested; waiting for Windows UAC.")?;
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(|e| {
        let code = if e.code().0 as u32 == 0x800704C7 {
            "UAC_CANCELLED"
        } else {
            "ELEVATION_FAILED"
        };
        failure(code, "Administrator launch was not completed", e)
    })?;
    let process = Handle(info.hProcess);
    if process.0.is_invalid() {
        return Err(failure(
            "ELEVATION_PROCESS",
            "Windows did not return the helper process",
            "No success has been assumed.",
        ));
    }
    if unsafe { WaitForSingleObject(process.0, 60000) } != WAIT_OBJECT_0 {
        return Err(failure("ELEVATION_TIMEOUT", "Administrator helper has not finished", "It may still be running. Refresh process status and read the proxy log before retrying."));
    }
    let mut exit = 0;
    unsafe { GetExitCodeProcess(process.0, &mut exit) }
        .map_err(|e| failure("ELEVATION_RESULT", "Cannot read helper exit status", e))?;
    let result_path = folder.join(format!("elevation-{request}.json"));
    let data = fs::read(&result_path).map_err(|e| {
        failure(
            "ELEVATION_RESULT",
            "Administrator helper returned no result",
            format!("Exit code {exit}; {e}. Use the same Windows account for UAC."),
        )
    })?;
    let result: Result<LaunchReceipt, AppError> = serde_json::from_slice(&data)
        .map_err(|e| failure("ELEVATION_RESULT", "Invalid administrator helper result", e))?;
    let _ = fs::remove_file(result_path);
    if exit != 0 && result.is_ok() {
        return Err(failure(
            "ELEVATION_RESULT",
            "Helper exited unexpectedly",
            exit,
        ));
    }
    result
}

pub fn cli_entry() -> Option<i32> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--proxy-elevated") {
        return None;
    }
    // No UI / WebView is created in the elevated helper.
    if args.len() != 4
        || args[2].len() > 64
        || !args[2].bytes().all(|b| b.is_ascii_digit() || b == b'-')
    {
        return Some(2);
    }
    let folder = match data_dir() {
        Ok(folder) => folder,
        Err(_) => return Some(2),
    };
    if !same_path(&folder, Path::new(&args[3])) || !is_elevated().unwrap_or(false) {
        return Some(3);
    }
    let result = super::launch(true);
    let exit = if result.is_ok() { 0 } else { 1 };
    if write_json(&folder.join(format!("elevation-{}.json", args[2])), &result).is_err() {
        return Some(4);
    }
    Some(exit)
}

struct Apartment;
impl Apartment {
    fn new() -> Result<Self, AppError> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .map_err(|e| failure("COM_INITIALIZE", "Cannot initialize Windows Shell", e))?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

fn shortcut(path: &Path, exe: &Path, arguments: &str, icon: &Path) -> Result<(), AppError> {
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| failure("SHORTCUT_CREATE", "Cannot create Windows shortcut", e))?;
    let persist: IPersistFile = link
        .cast()
        .map_err(|e| failure("SHORTCUT_CREATE", "Cannot persist Windows shortcut", e))?;
    let destination = HSTRING::from(path.as_os_str());
    if path.exists() {
        unsafe { persist.Load(&destination, STGM_READ) }
            .map_err(|e| failure("SHORTCUT_EXISTS", "Cannot inspect existing shortcut", e))?;
        let mut target = vec![0u16; 32768];
        unsafe { link.GetPath(&mut target, std::ptr::null_mut(), 0) }.map_err(|e| {
            failure(
                "SHORTCUT_EXISTS",
                "Cannot inspect existing shortcut target",
                e,
            )
        })?;
        let length = target.iter().position(|x| *x == 0).unwrap_or(target.len());
        if !same_path(Path::new(&String::from_utf16_lossy(&target[..length])), exe) {
            return Err(failure(
                "SHORTCUT_CONFLICT",
                "An unrelated shortcut already uses this name",
                path.display(),
            ));
        }
    }
    unsafe {
        link.SetPath(&HSTRING::from(exe.as_os_str()))
            .and_then(|_| link.SetArguments(&HSTRING::from(arguments)))
            .and_then(|_| {
                link.SetWorkingDirectory(&HSTRING::from(
                    exe.parent().unwrap_or(Path::new(".")).as_os_str(),
                ))
            })
            .and_then(|_| {
                link.SetDescription(w!(
                    "Codex Guard - launch Codex with process-only proxy settings"
                ))
            })
            .and_then(|_| link.SetIconLocation(&HSTRING::from(icon.as_os_str()), 0))
            .and_then(|_| link.SetShowCmd(SW_SHOWNORMAL))
            .and_then(|_| persist.Save(&destination, true))
    }
    .map_err(|e| failure("SHORTCUT_WRITE", "Cannot save Windows shortcut", e))
}

pub fn install_shortcuts() -> Result<Vec<String>, AppError> {
    let _lock = lock("proxy-operation.lock")?;
    let _apartment = Apartment::new()?;
    let desktop = known_folder(&FOLDERID_Desktop)?;
    let exe = std::env::current_exe()
        .map_err(|e| failure("SELF_PATH", "Cannot locate Codex Guard", e))?;
    let folder = data_dir()?;
    let mut installed = Vec::new();
    for (name, argument, filename, bytes) in [
        (
            "Codex Guard - 启动代理.lnk",
            "--proxy-launch",
            "proxy.ico",
            include_bytes!("../../icons/proxy.ico").as_slice(),
        ),
        (
            "Codex Guard - 管理员代理.lnk",
            "--proxy-launch-admin",
            "proxy-admin.ico",
            include_bytes!("../../icons/proxy-admin.ico").as_slice(),
        ),
    ] {
        let icon = folder.join(filename);
        fs::write(&icon, bytes)
            .map_err(|e| failure("SHORTCUT_ICON", "Cannot save shortcut icon", e))?;
        let path = desktop.join(name);
        shortcut(&path, &exe, argument, &icon).map_err(|e| {
            failure(
                &e.code,
                &e.message,
                format!("{}; already created: {installed:?}", e.detail),
            )
        })?;
        installed.push(path.display().to_string());
    }
    log(&format!(
        "Desktop shortcuts installed/refreshed: {installed:?}"
    ))?;
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argv_quoting_handles_spaces_quotes_and_trailing_slash() {
        assert_eq!(quote(r"C:\User Files\Guard"), "\"C:\\User Files\\Guard\"");
        assert_eq!(quote("x\"y"), "\"x\\\"y\"");
        assert_eq!(quote("C:\\"), "\"C:\\\\\"");
    }

    #[test]
    fn native_shortcut_roundtrip_in_temporary_folder() {
        let _apartment = Apartment::new().unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("代理测试.lnk");
        let exe = std::env::current_exe().unwrap();
        let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/proxy.ico");
        shortcut(&path, &exe, "--proxy-launch", &icon).unwrap();
        shortcut(&path, &exe, "--proxy-launch-admin", &icon).unwrap();
        let link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.unwrap();
        let persist: IPersistFile = link.cast().unwrap();
        unsafe { persist.Load(&HSTRING::from(path.as_os_str()), STGM_READ) }.unwrap();
        let mut args = vec![0u16; 1024];
        unsafe { link.GetArguments(&mut args) }.unwrap();
        let length = args.iter().position(|c| *c == 0).unwrap();
        assert_eq!(
            String::from_utf16_lossy(&args[..length]),
            "--proxy-launch-admin"
        );
        assert_eq!(
            shortcut(&path, Path::new(r"C:\Unrelated\Other.exe"), "", &icon)
                .unwrap_err()
                .code,
            "SHORTCUT_CONFLICT"
        );
    }
}
