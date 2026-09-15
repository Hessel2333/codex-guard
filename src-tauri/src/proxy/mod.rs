mod process;
mod settings;
mod shell;

use crate::error::AppError;
use serde::{Deserialize, Serialize};
pub use settings::{get_settings, save_settings, ProxySettings};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) fn failure(code: &str, message: &str, detail: impl std::fmt::Display) -> AppError {
    AppError::new(code, message, detail.to_string())
}
pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .replace('/', "\\")
        .eq_ignore_ascii_case(&b.to_string_lossy().replace('/', "\\"))
}
pub(crate) fn data_dir() -> Result<PathBuf, AppError> {
    #[cfg(test)]
    if let Some(path) = TEST_STATE.with(|state| state.borrow().clone()) {
        return Ok(path);
    }
    Ok(shell::local_data()?.join("CodexBootGuard"))
}

#[cfg(test)]
thread_local! { static TEST_STATE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) }; }

pub(crate) fn lock(name: &str) -> Result<fs::File, AppError> {
    let folder = data_dir()?;
    fs::create_dir_all(&folder).map_err(|e| {
        failure(
            "STATE_DIRECTORY",
            "Cannot create application data directory",
            e,
        )
    })?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(folder.join(name))
        .map_err(|e| failure("STATE_LOCK", "Cannot open operation lock", e))?;
    file.try_lock()
        .map_err(|e| failure("OPERATION_BUSY", "Another proxy operation is running", e))?;
    Ok(file)
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<(), AppError> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let data = serde_json::to_vec_pretty(value)
        .map_err(|e| failure("JSON_ENCODE", "Cannot encode application state", e))?;
    let mut file = fs::File::create(&temporary)
        .map_err(|e| failure("STATE_WRITE", "Cannot write application state", e))?;
    file.write_all(&data)
        .and_then(|_| file.sync_all())
        .map_err(|e| failure("STATE_WRITE", "Cannot flush application state", e))?;
    drop(file);
    fs::rename(&temporary, path)
        .map_err(|e| failure("STATE_COMMIT", "Cannot commit application state", e))
}

pub(crate) fn log(message: &str) -> Result<(), AppError> {
    let folder = data_dir()?;
    fs::create_dir_all(&folder)
        .map_err(|e| failure("LOG_DIRECTORY", "Cannot create log directory", e))?;
    let path = folder.join("proxy.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() > 512 * 1024) {
        fs::rename(&path, folder.join("proxy.previous.log"))
            .map_err(|e| failure("LOG_ROTATE", "Cannot rotate proxy log", e))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| failure("LOG_WRITE", "Cannot open proxy log", e))?;
    // JSON lines prevent paths / errors containing newlines from forging log entries.
    writeln!(
        file,
        "{}",
        serde_json::json!({"timestamp": now(), "message": message})
    )
    .map_err(|e| failure("LOG_WRITE", "Cannot write proxy log", e))
}

#[derive(Debug, Serialize)]
pub struct ProxyLogs {
    pub current: String,
    pub legacy: String,
    pub current_path: String,
    pub legacy_path: String,
}
fn tail(path: &Path) -> Result<String, AppError> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(failure("LOG_READ", "Cannot read proxy log", e)),
    };
    let length = file
        .metadata()
        .map_err(|e| failure("LOG_READ", "Cannot inspect log", e))?
        .len();
    file.seek(SeekFrom::Start(length.saturating_sub(64 * 1024)))
        .map_err(|e| failure("LOG_READ", "Cannot seek log", e))?;
    let mut bytes = Vec::new();
    file.take(64 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| failure("LOG_READ", "Cannot read log", e))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
pub fn get_logs() -> Result<ProxyLogs, AppError> {
    let current = data_dir()?.join("proxy.log");
    let legacy = shell::local_data()?
        .join("codex-proxy")
        .join("codex-proxy.log");
    Ok(ProxyLogs {
        current: tail(&current)?,
        legacy: tail(&legacy)?,
        current_path: current.display().to_string(),
        legacy_path: legacy.display().to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchReceipt {
    pub pid: u32,
    pub started_unix_ms: u64,
    pub executable: String,
    pub administrator: bool,
    pub launched_at_unix_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct ProxyStatus {
    pub settings: ProxySettings,
    pub settings_path: String,
    pub executable: Option<String>,
    pub discovery_error: Option<AppError>,
    pub processes: Vec<process::ProxyProcess>,
    pub administrator: bool,
    pub last_launch: Option<LaunchReceipt>,
    pub checked_at_unix_ms: u64,
}

fn desktop_candidate(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let parent = path
        .parent()
        .and_then(Path::file_name)
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    path.is_absolute()
        && matches!(name.as_str(), "chatgpt.exe" | "codex.exe")
        && !matches!(parent.as_str(), "resources" | "bin")
        && path.is_file()
}

fn discover(
    settings: &ProxySettings,
    processes: &[process::ProxyProcess],
) -> Result<PathBuf, AppError> {
    if let Some(path) = settings.app_path.as_deref().filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return if desktop_candidate(&path) {
            Ok(path)
        } else {
            Err(failure(
                "INVALID_APP_PATH",
                "Desktop executable override is not valid",
                "Choose an existing absolute ChatGPT.exe or Codex.exe path, not the bundled CLI.",
            ))
        };
    }
    let package_result = crate::windows::package();
    if let Ok(Some(package)) = &package_result {
        for name in ["ChatGPT.exe", "Codex.exe"] {
            let candidate = Path::new(&package.install_location).join("app").join(name);
            if desktop_candidate(&candidate) {
                return Ok(candidate);
            }
        }
    }
    for proc in processes {
        if let Some(path) = &proc.path {
            let candidate = PathBuf::from(path);
            if desktop_candidate(&candidate)
                && candidate
                    .parent()
                    .is_some_and(|p| p.join("resources").is_dir())
            {
                return Ok(candidate);
            }
        }
    }
    let local = shell::local_data()?;
    let mut roots = vec![local.join("Programs"), local];
    roots.extend(
        ["ProgramFiles", "ProgramFiles(x86)"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from),
    );
    for root in roots {
        for folder in ["ChatGPT Codex", "Codex"] {
            for name in ["ChatGPT.exe", "Codex.exe"] {
                let path = root.join(folder).join(name);
                if desktop_candidate(&path) {
                    return Ok(path);
                }
            }
        }
    }
    Err(failure(
        "CODEX_APP_NOT_FOUND",
        "Cannot locate Codex Desktop",
        package_result
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| {
                "Install OpenAI.Codex, or choose its desktop executable in Proxy settings.".into()
            }),
    ))
}

pub fn status() -> Result<ProxyStatus, AppError> {
    let settings = get_settings()?;
    let mut processes = process::list()?;
    let discovery = discover(&settings, &processes);
    if let Ok(exe) = &discovery {
        process::mark_managed(&mut processes, exe);
    }
    let last_launch = match fs::read(data_dir()?.join("proxy-last-launch.json")) {
        Ok(data) => Some(
            serde_json::from_slice(&data)
                .map_err(|e| failure("STATE_READ", "Cannot read last proxy launch", e))?,
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(failure("STATE_READ", "Cannot read last proxy launch", e)),
    };
    Ok(ProxyStatus {
        settings,
        settings_path: data_dir()?
            .join("proxy-settings.json")
            .display()
            .to_string(),
        executable: discovery.as_ref().ok().map(|p| p.display().to_string()),
        discovery_error: discovery.err(),
        processes,
        administrator: shell::is_elevated()?,
        last_launch,
        checked_at_unix_ms: now(),
    })
}

fn tracked_processes(exe: &Path) -> Result<Vec<process::ProxyProcess>, AppError> {
    let mut processes = process::list()?;
    process::mark_managed(&mut processes, exe);
    // Unknown paths might be an elevated existing desktop. Never terminate them by name.
    if processes.iter().any(|p| p.path.is_none()) {
        return Err(failure("PROCESS_ACCESS_DENIED", "Some Codex processes cannot be verified", "Close the inaccessible session manually, or use the explicit administrator launch option. No unverified process was stopped."));
    }
    Ok(processes.into_iter().filter(|p| p.managed).collect())
}

pub fn stop(confirmed: bool) -> Result<Vec<u32>, AppError> {
    if !confirmed {
        return Err(failure(
            "CONFIRMATION_REQUIRED",
            "Stopping Codex requires confirmation",
            "Unsaved work and active tasks may be interrupted.",
        ));
    }
    let _lock = lock("proxy-operation.lock")?;
    let result = (|| {
        let settings = get_settings()?;
        let exe = discover(&settings, &process::list()?)?;
        let targets = tracked_processes(&exe)?;
        log(&format!(
            "Stop requested; verified process count: {}",
            targets.len()
        ))?;
        let stopped = process::stop(&targets)?;
        log(&format!("Stopped process IDs: {stopped:?}"))?;
        Ok(stopped)
    })();
    if let Err(e) = &result {
        let _ = log(&format!("Stop error: {e}"));
    }
    result
}

pub(crate) fn child_command(exe: &Path, settings: &ProxySettings) -> Command {
    use std::os::windows::process::CommandExt;
    use windows::Win32::System::Threading::CREATE_NO_WINDOW;
    let mut command = Command::new(exe);
    // Suppress a console for console-subsystem targets; desktop GUI windows remain visible.
    command.creation_flags(CREATE_NO_WINDOW.0);
    // Windows environment keys are case-insensitive. Command owns a child-only environment block.
    command
        .env("HTTP_PROXY", &settings.http_proxy)
        .env("HTTPS_PROXY", &settings.http_proxy)
        .env("ALL_PROXY", &settings.all_proxy)
        .env("NO_PROXY", &settings.no_proxy);
    if let Some(folder) = exe.parent() {
        command.current_dir(folder);
        let cli = folder.join("resources").join("codex.exe");
        if cli.is_file() {
            command.env("CODEX_CLI_PATH", cli);
        }
    }
    command
}

pub fn launch(confirmed: bool) -> Result<LaunchReceipt, AppError> {
    // Start includes replacing the existing desktop session, as in the source launcher.
    if !confirmed {
        return Err(failure(
            "CONFIRMATION_REQUIRED",
            "Launching with proxy requires confirmation",
            "Existing verified Codex processes will be restarted.",
        ));
    }
    let _lock = lock("proxy-operation.lock")?;
    let result = (|| {
        let settings = get_settings()?;
        settings.validate()?;
        let exe = discover(&settings, &process::list()?)?; // Discover before stopping the running fallback.
        let administrator = shell::is_elevated()?;
        let targets = tracked_processes(&exe)?;
        log(&format!(
            "Proxy launch requested. Administrator: {administrator}. Executable: {}",
            exe.display()
        ))?;
        process::stop(&targets)?;
        let started = now();
        let mut child = child_command(&exe, &settings)
            .spawn()
            .map_err(|e| failure("LAUNCH_FAILED", "Cannot start Codex with proxy", e))?;
        log(&format!(
            "Created launcher PID {} with child-only proxy environment",
            child.id()
        ))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let candidates = process::list()?;
            if let Some(p) = candidates.iter().find(|p| {
                p.path
                    .as_deref()
                    .is_some_and(|p| same_path(Path::new(p), &exe))
                    && p.started_unix_ms
                        .is_some_and(|t| t >= started.saturating_sub(1000))
                    && process::is_from_launcher(p, child.id(), &candidates)
            }) {
                // Require the new desktop to survive an initial startup interval.
                if now().saturating_sub(started) >= 2000 {
                    let receipt = LaunchReceipt {
                        pid: p.pid,
                        started_unix_ms: p.started_unix_ms.unwrap_or(started),
                        executable: exe.display().to_string(),
                        administrator,
                        launched_at_unix_ms: started,
                    };
                    write_json(&data_dir()?.join("proxy-last-launch.json"), &receipt)?;
                    log(&format!(
                        "Proxy desktop running. PID: {}. Administrator: {administrator}",
                        receipt.pid
                    ))?;
                    return Ok(receipt);
                }
            }
            if std::time::Instant::now() >= deadline {
                let exit = child.try_wait().map_err(|e| {
                    failure("LAUNCH_STATUS", "Cannot inspect launcher exit status", e)
                })?;
                return Err(failure(
                    "LAUNCH_TIMEOUT",
                    "Codex did not remain running after launch",
                    format!(
                        "Launcher PID {}; exit status {exit:?}. See proxy log.",
                        child.id()
                    ),
                ));
            }
        }
    })();
    if let Err(e) = &result {
        let _ = log(&format!("Launch error: {e}"));
    }
    result
}

pub fn launch_admin(confirmed: bool) -> Result<LaunchReceipt, AppError> {
    if !confirmed {
        return Err(failure(
            "CONFIRMATION_REQUIRED",
            "Administrator launch requires confirmation",
            "Windows will display a UAC prompt.",
        ));
    }
    shell::launch_elevated()
}
pub fn install_shortcuts() -> Result<Vec<String>, AppError> {
    shell::install_shortcuts()
}
pub fn cli_entry() -> Option<i32> {
    shell::cli_entry()
}
pub fn startup_intent() -> Option<String> {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--proxy-launch-admin") {
        Some("admin".into())
    } else if args.iter().any(|a| a == "--proxy-launch") {
        Some("normal".into())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ResetTestState;
    impl Drop for ResetTestState {
        fn drop(&mut self) {
            TEST_STATE.with(|state| *state.borrow_mut() = None);
        }
    }

    #[test]
    fn all_destructive_entry_points_require_confirmation() {
        assert_eq!(launch(false).unwrap_err().code, "CONFIRMATION_REQUIRED");
        assert_eq!(
            launch_admin(false).unwrap_err().code,
            "CONFIRMATION_REQUIRED"
        );
        assert_eq!(stop(false).unwrap_err().code, "CONFIRMATION_REQUIRED");
    }

    #[test]
    fn native_proxy_launch_restart_stop_use_only_isolated_probe() {
        let temporary = tempfile::tempdir().unwrap();
        TEST_STATE.with(|state| *state.borrow_mut() = Some(temporary.path().join("state")));
        let _reset = ResetTestState;
        let app = temporary.path().join("app");
        fs::create_dir_all(app.join("resources")).unwrap();
        fs::write(
            app.join("resources").join("codex.exe"),
            "test path marker; never executed",
        )
        .unwrap();
        let exe = app.join("Codex.exe");
        let compiler = Command::new("rustc")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("test-fixtures/proxy_child.rs"))
            .args(["--edition=2021", "-o"])
            .arg(&exe)
            .output()
            .unwrap();
        assert!(
            compiler.status.success(),
            "{}",
            String::from_utf8_lossy(&compiler.stderr)
        );
        let before = std::env::var_os("HTTP_PROXY");
        let settings = ProxySettings {
            http_proxy: "http://127.0.0.1:7897".into(),
            app_path: Some(exe.display().to_string()),
            ..Default::default()
        };
        save_settings(settings.clone()).unwrap();
        assert_eq!(get_settings().unwrap(), settings);
        let first = launch(true).unwrap();
        let observed = fs::read_to_string(app.join("probe-env.txt")).unwrap();
        assert!(observed.contains("HTTP_PROXY=http://127.0.0.1:7897"));
        assert!(observed.contains("HTTPS_PROXY=http://127.0.0.1:7897"));
        assert!(observed.contains("ALL_PROXY=socks5://127.0.0.1:7890"));
        assert!(observed.contains("NO_PROXY=localhost,127.0.0.1,::1"));
        assert!(observed.contains(&format!(
            "CODEX_CLI_PATH={}",
            app.join("resources").join("codex.exe").display()
        )));
        let snapshot = status().unwrap();
        assert!(snapshot.processes.iter().filter(|p| p.managed).all(|p| p
            .path
            .as_deref()
            .is_some_and(|p| same_path(Path::new(p), &exe))));
        assert_eq!(snapshot.last_launch.unwrap().pid, first.pid);
        assert_eq!(stop(false).unwrap_err().code, "CONFIRMATION_REQUIRED");
        let second = launch(true).unwrap();
        assert_ne!(first.pid, second.pid);
        assert_eq!(stop(true).unwrap(), vec![second.pid]);
        assert!(!status().unwrap().processes.iter().any(|p| p.managed));
        assert_eq!(std::env::var_os("HTTP_PROXY"), before);
        assert!(tail(&data_dir().unwrap().join("proxy.log"))
            .unwrap()
            .contains("Proxy desktop running"));
    }
}
