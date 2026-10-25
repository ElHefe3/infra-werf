use atlas_core::{CONTRACT_VERSION, FoundationStatus};

#[tauri::command]
fn foundation_status() -> FoundationStatus {
    FoundationStatus {
        contract_version: CONTRACT_VERSION,
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        phase: "native shell; domain migration pending".into(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![foundation_status])
        .run(tauri::generate_context!())
        .expect("Atlas native runtime failed");
}
