mod workspace;

use atlas_core::*;
use atlas_core::{
    files::{FileAction, FileResult, FileSession},
    operations::OperationTicket,
    ssh::SshVerification,
};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use workspace::{Paths, Workspace};

fn local_desktop(window: &tauri::WebviewWindow) -> Result<(), AtlasError> {
    if !cfg!(desktop) || window.label() != "main" {
        return Err(AtlasError::new(ErrorCode::Denied));
    }
    let url = window
        .url()
        .map_err(|_| AtlasError::new(ErrorCode::Denied))?;
    let bundled = (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"))
        || (url.scheme() == "tauri" && url.host_str() == Some("localhost"));
    let development = cfg!(debug_assertions)
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
        && url.port() == Some(1420);
    if bundled || development {
        Ok(())
    } else {
        Err(AtlasError::new(ErrorCode::Denied))
    }
}
#[tauri::command]
fn ssh_prepare(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<OperationTicket, AtlasError> {
    local_desktop(&window)?;
    workspace.inner().prepare_ssh(&server_id)
}
#[tauri::command]
async fn ssh_verify(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    operation_id: OperationId,
) -> Result<SshVerification, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.verify_ssh(&server_id, &operation_id))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
fn ssh_cancel(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    operation_id: OperationId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.cancel_ssh(&operation_id)
}

#[tauri::command]
fn files_prepare(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<OperationTicket, AtlasError> {
    local_desktop(&window)?;
    workspace.inner().prepare_files(&server_id)
}
#[tauri::command]
async fn files_open(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    operation_id: OperationId,
) -> Result<FileSession, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.open_files(&server_id, &operation_id))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn files_read(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    action: FileAction,
) -> Result<FileResult, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.read_files(&server_id, &session_id, action)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn files_lock(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    session_id: SessionId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.lock_files(&session_id))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

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
        w.invalidate_server(&id);
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
        w.invalidate_server(&id);
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
        let workspace = Workspace::new(Paths::from_directory(app.path().app_data_dir()?));
        let weak = Arc::downgrade(&workspace);
        let handle = app.handle().clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(250));
                let Some(workspace) = weak.upgrade() else { break; };
                if workspace.is_closing() { break; }
                for session in workspace.reap_files() { let _ = handle.emit("atlas-session-closed", session); }
            }
        });
        app.manage(workspace);
        Ok(())
    })
    .on_window_event(|window, event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if !window.state::<Arc<Workspace>>().request_close() {
                api.prevent_close();
                let _ = window.emit("atlas-close-blocked", "Atlas is finishing a local operation or has an open SSH session. Lock open sessions and wait for work to finish, then close again.");
            }
        }
    })
    .invoke_handler(tauri::generate_handler![foundation_status, workspace_status, map_state, retry_workspace, add_server, update_server, remove_server, add_relationship, remove_relationship, import_reports, ssh_prepare, ssh_verify, ssh_cancel, files_prepare, files_open, files_read, files_lock])
    .run(tauri::generate_context!())
    .expect("Atlas native runtime failed");
}
