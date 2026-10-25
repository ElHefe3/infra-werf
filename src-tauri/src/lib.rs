mod workspace;

use atlas_core::*;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use workspace::{Paths, Workspace};

#[tauri::command]
fn foundation_status() -> FoundationStatus {
    FoundationStatus {
        contract_version: CONTRACT_VERSION,
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        phase: "desktop workspace; remote coordinator not yet available".into(),
    }
}

#[tauri::command]
fn workspace_status(workspace: State<'_, Arc<Workspace>>) -> Result<WorkspaceStatus, AtlasError> {
    workspace.status()
}

async fn run_store(
    workspace: Arc<Workspace>,
    action: impl FnOnce(&Workspace) -> Result<MapState, AtlasError> + Send + 'static,
) -> Result<MapState, AtlasError> {
    let operation = workspace.begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        action(&workspace)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

#[tauri::command]
async fn map_state(workspace: State<'_, Arc<Workspace>>) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), move |w| {
        w.with_store(|s| s.snapshot())
    })
    .await
}
#[tauri::command]
async fn retry_workspace(workspace: State<'_, Arc<Workspace>>) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), Workspace::retry).await
}
#[tauri::command]
async fn add_server(
    workspace: State<'_, Arc<Workspace>>,
    input: AddServer,
) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), move |w| {
        w.with_store(|s| {
            s.add_server(input)?;
            s.snapshot()
        })
    })
    .await
}
#[tauri::command]
async fn update_server(
    workspace: State<'_, Arc<Workspace>>,
    id: ServerId,
    input: UpdateServer,
) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), move |w| {
        w.with_store(|s| {
            s.update_server(&id, input)?;
            s.snapshot()
        })
    })
    .await
}
#[tauri::command]
async fn remove_server(
    workspace: State<'_, Arc<Workspace>>,
    id: ServerId,
) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), move |w| {
        w.with_store(|s| {
            s.remove_server(&id)?;
            s.snapshot()
        })
    })
    .await
}
#[tauri::command]
async fn add_relationship(
    workspace: State<'_, Arc<Workspace>>,
    input: AddRelationship,
) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), move |w| {
        w.with_store(|s| {
            s.add_relationship(input)?;
            s.snapshot()
        })
    })
    .await
}
#[tauri::command]
async fn remove_relationship(
    workspace: State<'_, Arc<Workspace>>,
    id: EdgeId,
) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), move |w| {
        w.with_store(|s| {
            s.remove_relationship(&id)?;
            s.snapshot()
        })
    })
    .await
}
#[tauri::command]
async fn import_reports(workspace: State<'_, Arc<Workspace>>) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), Workspace::import).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));
    builder.setup(|app| {
        app.manage(Workspace::new(Paths::from_directory(app.path().app_data_dir()?)));
        Ok(())
    })
    .on_window_event(|window, event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if !window.state::<Arc<Workspace>>().request_close() {
                api.prevent_close();
                let _ = window.emit("atlas-close-blocked", "Atlas is finishing a local operation. Wait for it to finish, then close again.");
            }
        }
    })
    .invoke_handler(tauri::generate_handler![foundation_status, workspace_status, map_state, retry_workspace, add_server, update_server, remove_server, add_relationship, remove_relationship, import_reports])
    .run(tauri::generate_context!())
    .expect("Atlas native runtime failed");
}
