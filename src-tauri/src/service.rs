use crate::{error::AppError, model::CodexStatus};

pub fn get_codex_status() -> Result<CodexStatus, AppError> {
    #[cfg(windows)]
    {
        crate::windows::detect()
    }
    #[cfg(not(windows))]
    {
        Err(AppError::new(
            "UNSUPPORTED_PLATFORM",
            "Windows 10 or Windows 11 is required",
            std::env::consts::OS,
        ))
    }
}
