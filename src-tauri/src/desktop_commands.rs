use crate::{
    desktop::{self, ApplyRequest, LaunchRequest, RestoreRequest, Selection},
    error::AppError,
};

async fn worker<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| AppError::new("WORKER_FAILED", "桌面配置操作未完成", e.to_string()))?
}
#[tauri::command]
pub async fn launch_desktop_config(
    request: LaunchRequest,
) -> Result<crate::proxy::LaunchReceipt, AppError> {
    worker(move || desktop::launch(request)).await
}
#[tauri::command]
pub async fn get_desktop_config_status(selection: Selection) -> Result<desktop::Status, AppError> {
    worker(move || desktop::status(selection)).await
}
#[tauri::command]
pub async fn preview_desktop_config(
    selection: Selection,
    revision: String,
    target_identity: String,
    edits: Vec<desktop::config::Edit>,
) -> Result<desktop::Preview, AppError> {
    worker(move || desktop::preview(selection, revision, target_identity, edits)).await
}
#[tauri::command]
pub async fn preview_desktop_restore(
    selection: Selection,
    revision: String,
    target_identity: String,
    snapshot_id: String,
) -> Result<desktop::Preview, AppError> {
    worker(move || desktop::preview_restore(selection, revision, target_identity, snapshot_id))
        .await
}
#[tauri::command]
pub async fn apply_desktop_config(
    request: ApplyRequest,
) -> Result<desktop::OperationResult, AppError> {
    worker(move || desktop::apply(request)).await
}
#[tauri::command]
pub async fn restore_desktop_config(
    request: RestoreRequest,
) -> Result<desktop::OperationResult, AppError> {
    worker(move || desktop::restore(request)).await
}
#[tauri::command]
pub async fn verify_desktop_config(
    selection: Selection,
    revision: String,
    target_identity: String,
) -> Result<desktop::Probe, AppError> {
    worker(move || desktop::verify(selection, revision, target_identity)).await
}
