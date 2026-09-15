// Read-only integration probe using exactly the same service as the Tauri command.
fn main() {
    match codex_guard_lib::service::get_codex_status() {
        Ok(status) => println!("{}", serde_json::to_string_pretty(&status).unwrap()),
        Err(error) => {
            eprintln!("{}", serde_json::to_string_pretty(&error).unwrap());
            std::process::exit(1);
        }
    }
}
