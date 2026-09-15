use super::{data_dir, failure, lock, write_json};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProxySettings {
    pub http_proxy: String,
    pub all_proxy: String,
    pub no_proxy: String,
    pub app_path: Option<String>,
}
impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            http_proxy: "http://127.0.0.1:7890".into(),
            all_proxy: "socks5://127.0.0.1:7890".into(),
            no_proxy: "localhost,127.0.0.1,::1".into(),
            app_path: None,
        }
    }
}
impl ProxySettings {
    pub fn validate(&self) -> Result<(), AppError> {
        for (name, value, schemes) in [
            (
                "HTTP / HTTPS proxy",
                &self.http_proxy,
                &["http", "https"][..],
            ),
            (
                "ALL_PROXY",
                &self.all_proxy,
                &["socks5", "socks5h", "socks4", "http", "https"][..],
            ),
        ] {
            if value.len() > 4096 || value.chars().any(char::is_control) || value.trim() != value {
                return Err(failure(
                    "INVALID_PROXY",
                    "Invalid proxy address",
                    format!("{name}: remove surrounding whitespace and control characters."),
                ));
            }
            let url = url::Url::parse(value).map_err(|_| {
                failure(
                    "INVALID_PROXY",
                    "Invalid proxy address",
                    format!("{name}: enter a full URL such as http://127.0.0.1:7890."),
                )
            })?;
            if !schemes.contains(&url.scheme())
                || url.host_str().is_none()
                || url.port_or_known_default().is_none_or(|p| p == 0)
                || url.query().is_some()
                || url.fragment().is_some()
                || !matches!(url.path(), "" | "/")
            {
                return Err(failure("INVALID_PROXY", "Invalid proxy address", format!("{name}: requires a supported scheme, host and valid port; no URL path/query/fragment.")));
            }
        }
        if self.no_proxy.len() > 8192 || self.no_proxy.chars().any(char::is_control) {
            return Err(failure(
                "INVALID_NO_PROXY",
                "Invalid proxy bypass list",
                "Use a comma-separated list without newlines.",
            ));
        }
        if let Some(path) = &self.app_path {
            if path.len() > 32767
                || path.chars().any(char::is_control)
                || !Path::new(path).is_absolute()
            {
                return Err(failure(
                    "INVALID_APP_PATH",
                    "Invalid desktop executable path",
                    "Use an absolute Windows path without quotes or control characters.",
                ));
            }
        }
        Ok(())
    }
}
pub fn get_settings() -> Result<ProxySettings, AppError> {
    let path = data_dir()?.join("proxy-settings.json");
    match fs::read(&path) {
        Ok(data) => {
            if data.len() > 64 * 1024 {
                return Err(failure(
                    "SETTINGS_READ",
                    "Proxy settings file is too large",
                    path.display(),
                ));
            }
            let settings: ProxySettings = serde_json::from_slice(&data)
                .map_err(|e| failure("SETTINGS_READ", "Cannot parse saved proxy settings", e))?;
            settings.validate()?;
            Ok(settings)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut settings = ProxySettings::default();
            if let Ok(value) = std::env::var("CODEX_HTTP_PROXY") {
                settings.http_proxy = value;
            }
            if let Ok(value) = std::env::var("CODEX_ALL_PROXY") {
                settings.all_proxy = value;
            }
            if let Ok(value) = std::env::var("CODEX_NO_PROXY") {
                settings.no_proxy = value;
            }
            settings.app_path = std::env::var("CODEX_APP").ok().filter(|v| !v.is_empty());
            settings.validate()?;
            Ok(settings)
        }
        Err(e) => Err(failure("SETTINGS_READ", "Cannot read proxy settings", e)),
    }
}
pub fn save_settings(mut settings: ProxySettings) -> Result<ProxySettings, AppError> {
    settings.http_proxy = settings.http_proxy.trim().into();
    settings.all_proxy = settings.all_proxy.trim().into();
    settings.no_proxy = settings.no_proxy.trim().into();
    settings.app_path = settings
        .app_path
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    settings.validate()?;
    let _lock = lock("proxy-operation.lock")?;
    write_json(&data_dir()?.join("proxy-settings.json"), &settings)?;
    super::log(
        "Proxy settings saved. No system proxy or user environment variables were changed.",
    )?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_defaults_are_valid() {
        assert!(ProxySettings::default().validate().is_ok());
    }
    #[test]
    fn rejects_bad_schemes_ports_and_control_characters() {
        for value in [
            "file:///C:/codex.exe",
            "127.0.0.1:7890",
            "http://localhost:0",
            "http://localhost:70000",
            "http://localhost:7890\n",
            "http://localhost:7890/path",
        ] {
            let settings = ProxySettings {
                http_proxy: value.into(),
                ..Default::default()
            };
            assert!(settings.validate().is_err(), "{value}");
        }
    }
    #[test]
    fn supports_ipv6_auth_and_socks_remote_dns() {
        let settings = ProxySettings {
            http_proxy: "http://user:pass@[::1]:7890".into(),
            all_proxy: "socks5h://127.0.0.1:7897".into(),
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
    }
    #[test]
    fn command_environment_does_not_modify_parent() {
        let before = std::env::var_os("HTTP_PROXY");
        let settings = ProxySettings::default();
        let command =
            super::super::child_command(Path::new(r"C:\Tools\Codex\Codex.exe"), &settings);
        let env: Vec<_> = command.get_envs().collect();
        assert!(env
            .iter()
            .any(|(k, v)| *k == "HTTP_PROXY"
                && *v == Some(std::ffi::OsStr::new(&settings.http_proxy))));
        assert!(env
            .iter()
            .any(|(k, v)| *k == "HTTPS_PROXY"
                && *v == Some(std::ffi::OsStr::new(&settings.http_proxy))));
        assert_eq!(std::env::var_os("HTTP_PROXY"), before);
    }
}
