use crate::{
    error::AppError,
    proxy::{self, LaunchReceipt, ProxyLogs, ProxySettings, ProxyStatus},
};

async fn worker<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| {
            AppError::new(
                "WORKER_FAILED",
                "Proxy operation worker failed",
                e.to_string(),
            )
        })?
}
#[tauri::command]
pub async fn get_proxy_status() -> Result<ProxyStatus, AppError> {
    worker(proxy::status).await
}
#[tauri::command]
pub async fn save_proxy_settings(settings: ProxySettings) -> Result<ProxySettings, AppError> {
    worker(move || proxy::save_settings(settings)).await
}
#[tauri::command]
pub async fn launch_codex_proxy(
    administrator: bool,
    confirmed: bool,
) -> Result<LaunchReceipt, AppError> {
    worker(move || {
        if administrator {
            proxy::launch_admin(confirmed)
        } else {
            proxy::launch(confirmed)
        }
    })
    .await
}
#[tauri::command]
pub async fn stop_codex_proxy(confirmed: bool) -> Result<Vec<u32>, AppError> {
    worker(move || proxy::stop(confirmed)).await
}
#[tauri::command]
pub async fn get_proxy_logs() -> Result<ProxyLogs, AppError> {
    worker(proxy::get_logs).await
}
#[tauri::command]
pub async fn install_proxy_shortcuts() -> Result<Vec<String>, AppError> {
    worker(proxy::install_shortcuts).await
}
#[tauri::command]
pub fn get_proxy_startup_intent() -> Option<String> {
    proxy::startup_intent()
}
