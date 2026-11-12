//! A fresh, isolated app-server, never the user's live connection. No thread or
//! turn RPC, auth file, provider config, plugin or MCP definition is copied.
use super::{
    config::{self, err},
    storage,
};
use crate::error::AppError;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

const MAX_LINE: usize = 512 * 1024;
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub cli_version: String,
    pub checked_at: u64,
    pub verified_keys: Vec<String>,
    pub scope: String,
    pub active_session_verified: bool,
}
struct ChildGuard {
    child: Child,
    #[cfg(windows)]
    job: windows::Win32::Foundation::HANDLE,
}
impl ChildGuard {
    fn spawn(command: &mut Command) -> Result<Self, AppError> {
        #[cfg(windows)]
        {
            use std::os::windows::{io::AsRawHandle, process::CommandExt};
            use windows::Win32::{
                Foundation::{CloseHandle, HANDLE},
                System::{
                    JobObjects::{
                        AssignProcessToJobObject, CreateJobObjectW,
                        JobObjectExtendedLimitInformation, SetInformationJobObject,
                        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                    },
                    Threading::CREATE_NO_WINDOW,
                },
            };
            command.creation_flags(CREATE_NO_WINDOW.0);
            let mut child = command
                .spawn()
                .map_err(|e| err("DESKTOP_ENGINE", "无法启动内置引擎", e))?;
            let job = match unsafe { CreateJobObjectW(None, None) } {
                Ok(job) => job,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(err("DESKTOP_ENGINE", "无法隔离检测进程", e));
                }
            };
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let assigned = unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of_val(&info) as u32,
                )
            }
            .and_then(|_| unsafe { AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())) });
            if let Err(e) = assigned {
                let _ = child.kill();
                let _ = child.wait();
                let _ = unsafe { CloseHandle(job) };
                return Err(err("DESKTOP_ENGINE", "无法隔离检测进程", e));
            }
            Ok(Self { child, job })
        }
        #[cfg(not(windows))]
        {
            Ok(Self {
                child: command
                    .spawn()
                    .map_err(|e| err("DESKTOP_ENGINE", "无法启动内置引擎", e))?,
            })
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.job);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn command(cli: &Path, home: &Path) -> Command {
    let mut command = Command::new(cli);
    command.current_dir(home).env_clear();
    // Only the OS variables necessary to start a native Windows executable.
    for key in [
        "SystemRoot",
        "WINDIR",
        "PATH",
        "TEMP",
        "TMP",
        "ComSpec",
        "PATHEXT",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("CODEX_HOME", home)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("LOCALAPPDATA", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}
fn lines(
    stdout: impl Read + Send + 'static,
) -> (
    mpsc::Receiver<Result<String, &'static str>>,
    std::thread::JoinHandle<()>,
) {
    let (tx, rx) = mpsc::sync_channel(16);
    let handle = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut total = 0;
        loop {
            let mut line = String::new();
            let read = reader
                .by_ref()
                .take((MAX_LINE + 1) as u64)
                .read_line(&mut line);
            match read {
                Ok(0) => break,
                Ok(n) if n <= MAX_LINE && total + n <= 2 * 1024 * 1024 => {
                    total += n;
                    if tx.send(Ok(line)).is_err() {
                        break;
                    }
                }
                _ => {
                    let _ = tx.send(Err("引擎返回内容过大或无法读取"));
                    break;
                }
            }
        }
    });
    (rx, handle)
}
fn receive(
    rx: &mpsc::Receiver<Result<String, &'static str>>,
    deadline: Instant,
) -> Result<String, AppError> {
    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(message)) => Err(err("DESKTOP_ENGINE", "引擎检测未完成", message)),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(err(
            "DESKTOP_TIMEOUT",
            "引擎检测超时",
            "检测进程已结束，配置未修改。",
        )),
        Err(_) => Err(err(
            "DESKTOP_ENGINE",
            "引擎提前退出",
            "请检查内置 CLI 是否支持 app-server 和 config/read。",
        )),
    }
}
fn version(cli: &Path, home: &Path) -> Result<String, AppError> {
    let mut cmd = command(cli, home);
    cmd.arg("--version").stdin(Stdio::null());
    let mut child = ChildGuard::spawn(&mut cmd)?;
    let (rx, reader) = lines(child.child.stdout.take().unwrap());
    let result = receive(&rx, Instant::now() + Duration::from_secs(5)).and_then(|line| {
        let line = line.trim();
        if line.starts_with("codex-cli ") && line.len() < 200 {
            Ok(line.to_owned())
        } else {
            Err(err(
                "DESKTOP_IDENTITY",
                "内置 CLI 身份无法确认",
                "版本输出不是预期的 Codex CLI。",
            ))
        }
    });
    drop(child);
    drop(rx);
    let _ = reader.join();
    result
}
pub fn verify(cli: &Path, bytes: Option<&[u8]>) -> Result<Evidence, AppError> {
    let source = config::parse(bytes)?;
    let mut fixture = toml_edit::DocumentMut::new();
    let mut expected = Vec::new();
    let mut verified_keys = Vec::new();
    for key in config::KEYS {
        if let Some(item) = source.get(key).filter(|item| !item.is_none()) {
            if let Some(text) = item.as_str() {
                fixture[key] = toml_edit::value(text);
                expected.push((key.to_owned(), text.to_owned()));
            } else {
                // Keep typed policies in the schema check too. Skipping a
                // malformed non-string value could otherwise report success.
                fixture[key] = item.clone();
            }
            verified_keys.push(key.to_owned());
        }
    }
    fixture["analytics"]["enabled"] = toml_edit::value(false);
    let home = tempfile::Builder::new()
        .prefix("codex-guard-engine-")
        .tempdir()
        .map_err(|e| err("DESKTOP_ENGINE", "无法创建隔离检测目录", e))?;
    storage::atomic_write(
        &home.path().join("config.toml"),
        fixture.to_string().as_bytes(),
    )?;
    let cli_version = version(cli, home.path())?;
    let mut cmd = command(cli, home.path());
    cmd.args(["app-server", "--listen", "stdio://", "--strict-config"]);
    let mut child = ChildGuard::spawn(&mut cmd)?;
    let (rx, reader) = lines(child.child.stdout.take().unwrap());
    let result = (|| {
        let input = child.child.stdin.as_mut().unwrap();
        let send = |input: &mut std::process::ChildStdin, value: Value| -> Result<(), AppError> {
            writeln!(input, "{value}")
                .and_then(|_| input.flush())
                .map_err(|_| err("DESKTOP_ENGINE", "无法连接引擎检测进程", "配置未修改。"))
        };
        send(
            input,
            json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"codex_guard_config_check","version":env!("CARGO_PKG_VERSION")}}}),
        )?;
        let deadline = Instant::now() + TIMEOUT;
        let mut initialized = false;
        loop {
            let raw = receive(&rx, deadline)?;
            let message: Value = serde_json::from_str(&raw).map_err(|_| {
                err(
                    "DESKTOP_ENGINE",
                    "引擎协议不兼容",
                    "未收到有效的 JSON 响应。",
                )
            })?;
            if message.get("error").is_some() {
                return Err(err(
                    "DESKTOP_ENGINE",
                    "引擎拒绝配置检测",
                    "当前内置 CLI 可能不支持所选配置或协议。",
                ));
            }
            if message["id"] == 1 && !initialized {
                initialized = true;
                send(input, json!({"method":"initialized","params":{}}))?;
                send(
                    input,
                    json!({"id":2,"method":"config/read","params":{"includeLayers":false}}),
                )?;
            }
            if message["id"] == 2 && initialized {
                let actual = message.pointer("/result/config").ok_or_else(|| {
                    err(
                        "DESKTOP_ENGINE",
                        "引擎未返回配置",
                        "无法确认所选配置是否受支持。",
                    )
                })?;
                for (key, value) in &expected {
                    if actual.get(key).and_then(Value::as_str) != Some(value.as_str()) {
                        return Err(err(
                            "DESKTOP_ENGINE_MISMATCH",
                            "引擎解析结果与所选配置不一致",
                            key,
                        ));
                    }
                }
                return Ok(Evidence {
                    cli_version,
                    checked_at: storage::now(),
                    verified_keys,
                    scope: "isolated_user_settings".into(),
                    active_session_verified: false,
                });
            }
        }
    })();
    drop(child);
    drop(rx);
    let _ = reader.join();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_reader_rejects_oversized_lines() {
        let (rx, reader) = lines(std::io::Cursor::new(vec![b'x'; MAX_LINE + 20]));
        assert!(receive(&rx, Instant::now() + Duration::from_secs(2)).is_err());
        drop(rx);
        reader.join().unwrap();
    }
    #[test]
    fn isolated_command_does_not_inherit_credentials_or_proxy() {
        let command = command(Path::new("fixture"), Path::new("home"));
        let env: Vec<_> = command
            .get_envs()
            .filter_map(|(k, v)| v.map(|_| k.to_string_lossy().to_uppercase()))
            .collect();
        assert!(!env.iter().any(|k| [
            "OPENAI_API_KEY",
            "CODEX_API_KEY",
            "HTTP_PROXY",
            "ACCESS_TOKEN"
        ]
        .contains(&k.as_str())));
        assert!(env.contains(&"CODEX_HOME".into()));
    }
    #[test]
    #[ignore = "explicit native integration: set GUARD_TEST_CODEX_CLI"]
    fn real_bundled_cli_reads_isolated_configuration() {
        let cli =
            std::env::var_os("GUARD_TEST_CODEX_CLI").expect("GUARD_TEST_CODEX_CLI is required");
        let evidence = verify(Path::new(&cli), Some(b"developer_instructions='fixture'\nmodel_reasoning_effort='low'\napproval_policy='on-request'\nsandbox_mode='read-only'\n")).unwrap();
        assert_eq!(evidence.verified_keys.len(), 4);
        assert!(!evidence.active_session_verified);
        assert!(
            verify(Path::new(&cli), Some(b"model_reasoning_effort = 123\n")).is_err(),
            "typed schema errors must never be skipped"
        );
        println!("{}", serde_json::to_string(&evidence).unwrap());
    }
}
