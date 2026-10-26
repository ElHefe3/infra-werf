mod workspace;

use atlas_core::files::{
    ActionPassword, ElevationPreview, ElevationReceipt, LocalSelection, ProtectedPlacementReceipt,
    ProtectedPlacementRequest, ProtectedPlacementStage, TransferDirection, TransferView,
};
use atlas_core::*;
use atlas_core::{
    files::{
        FileAction, FileChange, FileMutationPreview, FileMutationReceipt, FileResult, FileSession,
    },
    health::{HealthProbeConfig, HealthSnapshot},
    operations::OperationTicket,
    ssh::SshVerification,
};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
#[cfg(desktop)]
use tauri_plugin_dialog::DialogExt;
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
fn health_configure(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    config: HealthProbeConfig,
) -> Result<HealthSnapshot, AtlasError> {
    local_desktop(&window)?;
    workspace.inner().configure_health(&server_id, config)
}
#[tauri::command]
fn health_status(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<HealthSnapshot, AtlasError> {
    local_desktop(&window)?;
    workspace.inner().health_status(&server_id)
}
#[tauri::command]
fn health_cancel(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.inner().cancel_health(&server_id)
}

#[tauri::command]
async fn inventory_refresh(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<InventoryRefreshResult, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.refresh_inventory(&server_id))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
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
async fn workload_inspect(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    workload_id: String,
) -> Result<atlas_core::workloads::WorkloadInspection, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.inspect_workload(&server_id, &session_id, &workload_id)
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
async fn files_terminal_start(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    cols: u16,
    rows: u16,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.start_terminal(&server_id, &session_id, cols, rows)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

#[tauri::command]
async fn files_terminal_poll(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
) -> Result<atlas_core::files::TerminalPoll, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.poll_terminal(&server_id, &session_id))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

#[tauri::command]
async fn files_terminal_write(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    data: Vec<u8>,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.write_terminal(&server_id, &session_id, &data)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

#[tauri::command]
async fn files_terminal_resize(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    cols: u16,
    rows: u16,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.resize_terminal(&server_id, &session_id, cols, rows)
    })
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
async fn files_pick_upload(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
) -> Result<Option<LocalSelection>, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    let admission = workspace.begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _admission = admission;
        workspace.picker_access(&server_id, &session_id)?;
        #[cfg(desktop)]
        {
            let selected = window
                .dialog()
                .file()
                .set_parent(&window)
                .set_title("Choose file to upload")
                .blocking_pick_file();
            selected
                .map(|path| {
                    workspace.select_local(
                        &server_id,
                        &session_id,
                        path.into_path()
                            .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?,
                        TransferDirection::Upload,
                    )
                })
                .transpose()
        }
        #[cfg(not(desktop))]
        {
            Err(AtlasError::new(ErrorCode::Denied))
        }
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn files_pick_download(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    resource_id: String,
) -> Result<Option<LocalSelection>, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    let admission = workspace.begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _admission = admission;
        let name = workspace.download_name(&server_id, &session_id, &resource_id)?;
        #[cfg(desktop)]
        {
            let selected = window
                .dialog()
                .file()
                .set_parent(&window)
                .set_file_name(name)
                .set_title("Save downloaded file")
                .blocking_save_file();
            selected
                .map(|path| {
                    workspace.select_local(
                        &server_id,
                        &session_id,
                        path.into_path()
                            .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?,
                        TransferDirection::Download,
                    )
                })
                .transpose()
        }
        #[cfg(not(desktop))]
        {
            let _ = name;
            Err(AtlasError::new(ErrorCode::Denied))
        }
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
fn files_selection_discard(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    grant_id: String,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.discard_selection(&grant_id)
}
#[tauri::command]
async fn files_transfer(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    resource_id: String,
    grant_id: String,
) -> Result<TransferView, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.enqueue_transfer(&server_id, &session_id, &resource_id, &grant_id)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn files_stage_protected(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    resource_id: String,
    grant_id: String,
) -> Result<ProtectedPlacementStage, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.stage_protected_placement(&server_id, &session_id, &resource_id, &grant_id)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
fn transfer_list(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<Vec<TransferView>, AtlasError> {
    local_desktop(&window)?;
    workspace.transfers()
}
#[tauri::command]
fn transfer_cancel(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    operation_id: OperationId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.cancel_transfer(&operation_id)
}
#[tauri::command]
async fn files_release(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    session_id: SessionId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.release_files(&session_id))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

#[tauri::command]
async fn files_preview(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    change: FileChange,
) -> Result<FileMutationPreview, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.preview_change(&server_id, &session_id, change)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn files_commit(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    operation_id: OperationId,
) -> Result<FileMutationReceipt, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.commit_change(&server_id, &session_id, &operation_id)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn files_discard(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    operation_id: OperationId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.discard_change(&server_id, &session_id, &operation_id)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

#[tauri::command]
async fn elevation_preview(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
) -> Result<ElevationPreview, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.preview_elevation(&server_id, &session_id)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn elevation_confirm(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    operation_id: OperationId,
    password: String,
) -> Result<ElevationReceipt, AtlasError> {
    let password = ActionPassword::from(password);
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.confirm_elevation(&server_id, &session_id, &operation_id, password)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn elevation_cancel(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    operation_id: OperationId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.cancel_elevation(&server_id, &session_id, &operation_id)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn protected_placement_preview(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    request: ProtectedPlacementRequest,
) -> Result<ElevationPreview, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.preview_protected_placement(&server_id, &session_id, request)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn protected_placement_confirm(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    operation_id: OperationId,
    password: String,
) -> Result<ProtectedPlacementReceipt, AtlasError> {
    let password = ActionPassword::from(password);
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.confirm_protected_placement(&server_id, &session_id, &operation_id, password)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn protected_placement_cancel(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    resource_id: String,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.cancel_protected_placement(&server_id, &session_id, &resource_id)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
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
        let state = action(&workspace)?;
        workspace.apply_health(state)
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
        let host_changed = input.host.is_some();
        let state = w.with_store(|s| {
            s.update_server(&id, input)?;
            s.snapshot()
        })?;
        w.invalidate_server(&id);
        if host_changed {
            w.cancel_health(&id)?;
        }
        Ok(state)
    })
    .await
}
#[tauri::command]
async fn remove_server(
    workspace: State<'_, Arc<Workspace>>,
    id: ServerId,
) -> Result<MapState, AtlasError> {
    run_store(workspace.inner().clone(), move |w| {
        let state = w.with_store(|s| {
            s.remove_server(&id)?;
            s.snapshot()
        })?;
        w.invalidate_server(&id);
        w.cancel_health(&id)?;
        Ok(state)
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
    let builder =
        builder
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_single_instance::init(|app, _, _| {
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
                workspace.tick_health();
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
    .invoke_handler(tauri::generate_handler![foundation_status, workspace_status, map_state, retry_workspace, add_server, update_server, remove_server, add_relationship, remove_relationship, import_reports, ssh_prepare, ssh_verify, ssh_cancel, health_configure, health_status, health_cancel, inventory_refresh, files_prepare, files_open, files_read, workload_inspect, files_lock, files_terminal_start, files_terminal_poll, files_terminal_write, files_terminal_resize, files_preview, files_commit, files_discard, files_release, files_pick_upload, files_pick_download, files_selection_discard, files_transfer, files_stage_protected, transfer_list, transfer_cancel, elevation_preview, elevation_confirm, elevation_cancel, protected_placement_preview, protected_placement_confirm, protected_placement_cancel])
    .run(tauri::generate_context!())
    .expect("Atlas native runtime failed");
}
