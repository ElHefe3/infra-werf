use serde::Serialize;

#[derive(Serialize)]
struct FoundationStatus {
    version: &'static str,
    platform: &'static str,
    phase: &'static str,
}

#[tauri::command]
fn foundation_status() -> FoundationStatus {
    FoundationStatus {
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        phase: "native shell; domain migration pending",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![foundation_status])
        .run(tauri::generate_context!())
        .expect("Atlas native runtime failed");
}
