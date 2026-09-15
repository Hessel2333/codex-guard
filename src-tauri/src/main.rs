// Both development and release windows start without allocating a console.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    if let Some(code) = codex_guard_lib::proxy::cli_entry() {
        std::process::exit(code);
    }
    codex_guard_lib::run()
}
