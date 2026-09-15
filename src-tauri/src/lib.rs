pub mod error;
pub mod model;
pub mod proxy;
mod proxy_commands;
pub mod service;
#[cfg(windows)]
mod windows;

use error::AppError;
use model::CodexStatus;

#[tauri::command]
async fn get_codex_status() -> Result<CodexStatus, AppError> {
    // AppX and file I/O must never block the WebView event loop.
    tauri::async_runtime::spawn_blocking(service::get_codex_status)
        .await
        .map_err(|e| AppError::new("WORKER_FAILED", "Detection worker failed", e.to_string()))?
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_codex_status,
            proxy_commands::get_proxy_status,
            proxy_commands::save_proxy_settings,
            proxy_commands::launch_codex_proxy,
            proxy_commands::stop_codex_proxy,
            proxy_commands::get_proxy_logs,
            proxy_commands::install_proxy_shortcuts,
            proxy_commands::get_proxy_startup_intent,
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Codex Guard");
}
