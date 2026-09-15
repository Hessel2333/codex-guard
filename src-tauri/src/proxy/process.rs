use super::{failure, same_path};
use crate::error::AppError;
use serde::Serialize;
use std::{path::Path, time::Duration};
use windows::{
    core::{BOOL, PWSTR},
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{
                GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
                WaitForSingleObject, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
            },
        },
        UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
    },
};

pub(super) struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyProcess {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub path: Option<String>,
    pub started_unix_ms: Option<u64>,
    pub managed: bool,
    pub error: Option<String>,
}

fn image(handle: HANDLE) -> Result<String, AppError> {
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|e| failure("PROCESS_PATH", "Cannot verify process executable", e))?;
    Ok(String::from_utf16_lossy(&buffer[..length as usize]))
}
fn created(handle: HANDLE) -> Result<u64, AppError> {
    let (mut creation, mut exit, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) }
        .map_err(|e| failure("PROCESS_TIME", "Cannot verify process start time", e))?;
    let ticks = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
    Ok((ticks / 10000).saturating_sub(11644473600000))
}

pub fn list() -> Result<Vec<ProxyProcess>, AppError> {
    let mut session = 0;
    unsafe { ProcessIdToSessionId(std::process::id(), &mut session) }
        .map_err(|e| failure("PROCESS_SESSION", "Cannot read Windows session", e))?;
    let snapshot = Handle(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
            .map_err(|e| failure("PROCESS_SNAPSHOT", "Cannot enumerate processes", e))?,
    );
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    let mut result = Vec::new();
    loop {
        match next {
            Ok(()) => {}
            Err(e) if e.code().0 as u32 == 0x80070012 => break, // ERROR_NO_MORE_FILES
            Err(e) => {
                return Err(failure(
                    "PROCESS_ENUMERATION",
                    "Cannot enumerate processes",
                    e,
                ))
            }
        }
        let length = entry
            .szExeFile
            .iter()
            .position(|x| *x == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
        let mut target_session = u32::MAX;
        if matches!(
            name.to_lowercase().as_str(),
            "chatgpt.exe" | "codex.exe" | "codex-code-mode-host.exe"
        ) && unsafe { ProcessIdToSessionId(entry.th32ProcessID, &mut target_session) }.is_ok()
            && target_session == session
        {
            let mut info = ProxyProcess {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                name,
                path: None,
                started_unix_ms: None,
                managed: false,
                error: None,
            };
            match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, info.pid) } {
                Ok(raw) => {
                    let handle = Handle(raw);
                    match image(handle.0) {
                        Ok(path) => info.path = Some(path),
                        Err(e) => info.error = Some(e.to_string()),
                    }
                    match created(handle.0) {
                        Ok(time) => info.started_unix_ms = Some(time),
                        Err(e) => info.error = Some(e.to_string()),
                    }
                }
                Err(e) => info.error = Some(e.to_string()),
            }
            result.push(info);
        }
        next = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    Ok(result)
}

fn under_directory(path: &Path, directory: &Path) -> bool {
    // Component boundary prevents C:\CodexEvil from matching C:\Codex.
    let path = path.to_string_lossy().replace('/', "\\").to_lowercase();
    let directory = directory
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase();
    path.starts_with(&(directory.trim_end_matches('\\').to_owned() + "\\"))
}
pub fn mark_managed(processes: &mut [ProxyProcess], exe: &Path) {
    let Some(directory) = exe.parent() else {
        return;
    };
    for process in processes {
        process.managed = process.path.as_deref().is_some_and(|p| {
            same_path(Path::new(p), exe)
                || under_directory(Path::new(p), &directory.join("resources"))
        }) && process.started_unix_ms.is_some();
    }
}

pub fn is_from_launcher(candidate: &ProxyProcess, launcher_pid: u32, all: &[ProxyProcess]) -> bool {
    let mut current = candidate;
    for _ in 0..=all.len() {
        if current.pid == launcher_pid || current.parent_pid == launcher_pid {
            return true;
        }
        let Some(parent) = all.iter().find(|p| p.pid == current.parent_pid) else {
            return false;
        };
        if parent.started_unix_ms > current.started_unix_ms {
            return false;
        }
        current = parent;
    }
    false
}

unsafe extern "system" fn close_window(hwnd: HWND, parameter: LPARAM) -> BOOL {
    // The stack-owned PID slice lives for the complete synchronous EnumWindows call.
    let ids = &*(parameter.0 as *const Vec<u32>);
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if ids.contains(&pid) {
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
    BOOL(1)
}

pub fn stop(targets: &[ProxyProcess]) -> Result<Vec<u32>, AppError> {
    // Acquire all handles before sending any close message. Verify image and creation time
    // on each handle so a recycled PID cannot target another process.
    let mut verified = Vec::new();
    for process in targets {
        if !process.managed {
            return Err(failure(
                "UNVERIFIED_PROCESS",
                "Refusing to stop an unrelated process",
                process.pid,
            ));
        }
        match unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                false,
                process.pid,
            )
        } {
            Ok(raw) => {
                let handle = Handle(raw);
                let path = image(handle.0)?;
                let start = created(handle.0)?;
                if !process
                    .path
                    .as_deref()
                    .is_some_and(|p| same_path(Path::new(p), Path::new(&path)))
                    || process.started_unix_ms != Some(start)
                {
                    return Err(failure(
                        "PROCESS_CHANGED",
                        "Process identity changed; refresh before retrying",
                        process.pid,
                    ));
                }
                verified.push((process.pid, handle));
            }
            Err(e) if e.code().0 as u32 == 0x80070057 => {} // Already exited / invalid PID.
            Err(e) => {
                return Err(failure(
                    "STOP_ACCESS_DENIED",
                    "Cannot stop verified Codex process",
                    format!(
                        "PID {}: {e}. Administrator permission may be required.",
                        process.pid
                    ),
                ))
            }
        }
    }
    let ids: Vec<u32> = verified.iter().map(|(pid, _)| *pid).collect();
    if ids.is_empty() {
        return Ok(ids);
    }
    unsafe { EnumWindows(Some(close_window), LPARAM(&ids as *const Vec<u32> as isize)) }
        .map_err(|e| failure("CLOSE_WINDOWS", "Cannot request graceful Codex shutdown", e))?;
    std::thread::sleep(Duration::from_secs(2));
    for (pid, handle) in verified {
        if unsafe { WaitForSingleObject(handle.0, 0) } != WAIT_OBJECT_0 {
            unsafe { TerminateProcess(handle.0, 1) }.map_err(|e| {
                failure(
                    "STOP_FAILED",
                    "Cannot terminate Codex process",
                    format!("PID {pid}: {e}"),
                )
            })?;
            if unsafe { WaitForSingleObject(handle.0, 5000) } != WAIT_OBJECT_0 {
                return Err(failure("STOP_TIMEOUT", "Codex process did not exit", pid));
            }
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_sibling_executables_are_not_managed() {
        let row = |path: &str| ProxyProcess {
            pid: 1,
            parent_pid: 0,
            name: "codex.exe".into(),
            path: Some(path.into()),
            started_unix_ms: Some(1),
            managed: false,
            error: None,
        };
        let mut rows = vec![
            row(r"C:\Codex.exe"),
            row(r"C:\Unrelated\codex.exe"),
            row(r"C:\resources\codex.exe"),
        ];
        mark_managed(&mut rows, Path::new(r"C:\Codex.exe"));
        assert!(rows[0].managed && !rows[1].managed && rows[2].managed);
    }
    #[test]
    fn newly_started_unrelated_session_is_not_accepted_as_proxy_launch() {
        let row = |pid, parent_pid| ProxyProcess {
            pid,
            parent_pid,
            name: "Codex.exe".into(),
            path: None,
            started_unix_ms: Some(1000),
            managed: false,
            error: None,
        };
        let rows = vec![row(10, 1), row(20, 10), row(30, 20), row(40, 1)];
        assert!(is_from_launcher(&rows[2], 10, &rows));
        assert!(!is_from_launcher(&rows[3], 10, &rows));
    }
    #[test]
    fn process_directory_requires_component_boundary() {
        assert!(under_directory(
            Path::new(r"c:\Codex\resources\codex.exe"),
            Path::new(r"C:\Codex")
        ));
        assert!(!under_directory(
            Path::new(r"C:\CodexOther\codex.exe"),
            Path::new(r"C:\Codex")
        ));
    }
    #[test]
    fn unverified_process_stop_fails_before_any_os_action() {
        let process = ProxyProcess {
            pid: std::process::id(),
            parent_pid: 0,
            name: "codex.exe".into(),
            path: None,
            started_unix_ms: None,
            managed: false,
            error: None,
        };
        assert_eq!(stop(&[process]).unwrap_err().code, "UNVERIFIED_PROCESS");
    }
}
