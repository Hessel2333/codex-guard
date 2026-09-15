// Harmless native lifecycle probe. Never starts Codex or connects to a proxy.
fn main() {
    #[link(name = "kernel32")]
    extern "system" { fn GetConsoleWindow() -> *mut std::ffi::c_void; }
    // This fixture deliberately uses the console subsystem to verify CREATE_NO_WINDOW.
    assert!(unsafe { GetConsoleWindow() }.is_null(), "Child unexpectedly has a console");
    let directory = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let values: Vec<String> = ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY", "CODEX_CLI_PATH"]
        .iter().map(|name| format!("{name}={}", std::env::var(name).unwrap_or_default())).collect();
    std::fs::write(directory.join("probe-env.txt"), values.join("\n")).unwrap();
    // A finite lifetime also cleans up the probe if a test assertion fails.
    std::thread::sleep(std::time::Duration::from_secs(20));
}
