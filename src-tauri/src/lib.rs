mod apps;
mod workspace;

use atlas_core::files::{
    ActionPassword, ElevationPreview, ElevationReceipt, LocalSelection, ProtectedPlacementReceipt,
    ProtectedPlacementRequest, ProtectedPlacementStage, TransferDirection, TransferView,
};
use atlas_core::monitoring::{MonitoringPolicy, MonitoringState};

#[tauri::command]
async fn monitoring_accept_identity(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    previous: String,
    accepted: String,
) -> Result<MonitoringState, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = workspace.begin()?;
        workspace.accept_live_identity(&server_id, &previous, &accepted)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
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

#[tauri::command]
fn monitoring_state(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<MonitoringState, AtlasError> {
    local_desktop(&window)?;
    workspace.monitoring_state()
}
#[tauri::command]
async fn monitoring_configure(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    policy: MonitoringPolicy,
) -> Result<MonitoringState, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = workspace.begin()?;
        workspace.set_monitoring(&server_id, policy)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
fn monitoring_history_limit(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    limit: u32,
) -> Result<MonitoringState, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.history_limit(limit)
}
#[tauri::command]
async fn monitoring_discover(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<MonitoringState, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = workspace.begin()?;
        workspace.discover_configured()
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

/// Events go to InfraWerf's own interface only, never to a plugin view in
/// the same window.
fn main_webview() -> tauri::EventTarget {
    tauri::EventTarget::Webview {
        label: "main".into(),
    }
}

/// Plugin view tabs for one server (T047).
#[tauri::command]
fn plugin_views(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<atlas_core::plugin_host::PluginViewsState, AtlasError> {
    local_desktop(&window)?;
    workspace.plugin_views(&server_id)
}
/// Opens a plugin view over the given area of the main window.
#[tauri::command]
async fn plugin_view_open(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    plugin_id: String,
    contribution_id: String,
    bounds: workspace::ViewBounds,
) -> Result<String, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    let main = window.window();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.open_plugin_view(&main, &server_id, &plugin_id, &contribution_id, bounds)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
/// Moves a plugin view, or hides it while InfraWerf shows something over it.
#[tauri::command]
fn plugin_view_bounds(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    label: String,
    bounds: Option<workspace::ViewBounds>,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.plugin_view_bounds(&label, bounds)
}
/// Closes a plugin view: closing its tab, Lock, or closing the workspace.
#[tauri::command]
fn plugin_view_close(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    label: String,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.close_plugin_view(&label);
    Ok(())
}
/// The plugin views open now.
#[tauri::command]
fn plugin_views_open(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<Vec<serde_json::Value>, AtlasError> {
    local_desktop(&window)?;
    Ok(workspace.open_plugin_views())
}
/// The owner's answer to a view's request, given in InfraWerf's own dialog.
#[tauri::command]
async fn plugin_view_answer(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    id: String,
    run: bool,
) -> Result<atlas_core::plugin_view::ViewActionState, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.answer_view_action(&id, run))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

/// Admits a command only from InfraWerf's own interface: the webview
/// labelled "main" in the window labelled "main", showing the bundled app.
/// Plugin views are child webviews of the same window (T047), so the
/// window's label alone is not enough.
pub(crate) fn local_desktop(window: &tauri::Webview) -> Result<(), AtlasError> {
    if !cfg!(desktop) || window.label() != "main" || window.window().label() != "main" {
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
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<OperationTicket, AtlasError> {
    local_desktop(&window)?;
    workspace.inner().prepare_ssh(&server_id)
}
#[tauri::command]
async fn ssh_verify(
    window: tauri::Webview,
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
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    operation_id: OperationId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.cancel_ssh(&operation_id)
}

#[tauri::command]
async fn health_configure(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    config: HealthProbeConfig,
) -> Result<HealthSnapshot, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.configure_health(&server_id, config))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
fn health_status(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<HealthSnapshot, AtlasError> {
    local_desktop(&window)?;
    workspace.inner().health_status(&server_id)
}
#[tauri::command]
fn health_cancel(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.inner().cancel_health(&server_id)
}

#[tauri::command]
async fn inventory_refresh(
    window: tauri::Webview,
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
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<OperationTicket, AtlasError> {
    local_desktop(&window)?;
    workspace.inner().prepare_files(&server_id)
}
#[tauri::command]
async fn files_open(
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
async fn workload_control_preview(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    workload_id: String,
    action: atlas_core::workloads::WorkloadAction,
) -> Result<atlas_core::workloads::WorkloadControlPreview, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.preview_workload_control(&server_id, &session_id, &workload_id, action)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

#[tauri::command]
async fn workload_control_confirm(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    session_id: SessionId,
    operation_id: OperationId,
    password: Option<String>,
) -> Result<atlas_core::workloads::WorkloadControlReceipt, AtlasError> {
    let password = password.map(ActionPassword::from);
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.confirm_workload_control(&server_id, &session_id, &operation_id, password)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
async fn files_lock(
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
fn foundation_status(window: tauri::Webview) -> Result<FoundationStatus, AtlasError> {
    local_desktop(&window)?;
    Ok(FoundationStatus {
        contract_version: CONTRACT_VERSION,
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        phase: "desktop workspace; remote coordinator not yet available".into(),
    })
}

#[tauri::command]
async fn files_pick_upload(
    window: tauri::Webview,
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
                .set_parent(&window.window())
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
    window: tauri::Webview,
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
                .set_parent(&window.window())
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
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    grant_id: String,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.discard_selection(&grant_id)
}
#[tauri::command]
async fn files_transfer(
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<Vec<TransferView>, AtlasError> {
    local_desktop(&window)?;
    workspace.transfers()
}
#[tauri::command]
fn transfer_cancel(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    operation_id: OperationId,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.cancel_transfer(&operation_id)
}
#[tauri::command]
async fn files_release(
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
    window: tauri::Webview,
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
fn plugins_state(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<atlas_core::plugin_host::PluginsState, AtlasError> {
    local_desktop(&window)?;
    workspace.plugins_state()
}
#[tauri::command]
fn plugin_enable(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    plugin_id: String,
    trust: atlas_core::plugin_host::TrustLevel,
) -> Result<atlas_core::plugin_host::PluginsState, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.plugin_enable(&plugin_id, trust)
}
/// Installs a plugin from a folder, an archive or an HTTPS address (T045).
/// It arrives disabled.
#[tauri::command]
async fn plugin_install(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    request: atlas_core::plugin_install::InstallRequest,
) -> Result<atlas_core::plugin_install::InstallReport, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    let admission = workspace.begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _admission = admission;
        workspace.plugin_install(&request)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
/// Checks an update for an installed plugin and shows what it changes (T046).
#[tauri::command]
async fn plugin_update_check(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    plugin_id: String,
    request: atlas_core::plugin_install::InstallRequest,
) -> Result<atlas_core::plugin_install::UpdatePreview, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    let admission = workspace.begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _admission = admission;
        workspace.plugin_update_check(&plugin_id, &request)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
/// Applies a checked update (T046).
#[tauri::command]
fn plugin_update_apply(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    token: String,
    allow_downgrade: bool,
) -> Result<atlas_core::plugin_install::UpdatePreview, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.plugin_update_apply(&token, allow_downgrade)
}
#[tauri::command]
fn plugin_update_discard(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    token: String,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.plugin_update_discard(&token)
}
/// Removes a plugin folder, its permissions and its install record (T045).
#[tauri::command]
fn plugin_remove(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    folder: String,
) -> Result<atlas_core::plugin_host::PluginsState, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.plugin_remove(&folder)
}
/// Lets the owner choose a plugin folder or archive on this PC.
#[tauri::command]
async fn plugin_pick_source(
    window: tauri::Webview,
    folder: bool,
) -> Result<Option<String>, AtlasError> {
    local_desktop(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(desktop)]
        {
            let dialog = window.dialog().file().set_parent(&window.window());
            let picked = if folder {
                dialog
                    .set_title("Choose a plugin folder")
                    .blocking_pick_folder()
            } else {
                dialog
                    .set_title("Choose a plugin archive")
                    .add_filter("Plugin archive", &["zip", "gz", "tgz"])
                    .blocking_pick_file()
            };
            picked
                .map(|path| {
                    path.into_path()
                        .map(|path| path.to_string_lossy().into_owned())
                        .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))
                })
                .transpose()
        }
        #[cfg(not(desktop))]
        {
            let _ = (window, folder);
            Ok(None)
        }
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
#[tauri::command]
fn plugin_disable(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    plugin_id: String,
) -> Result<atlas_core::plugin_host::PluginsState, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.plugin_disable(&plugin_id)
}
#[tauri::command]
fn plugin_set_grant(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    plugin_id: String,
    server_id: ServerId,
    capabilities: Vec<atlas_core::extensions::Capability>,
) -> Result<atlas_core::plugin_host::PluginsState, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.plugin_set_grant(&plugin_id, &server_id, &capabilities)
}
#[tauri::command]
fn plugin_activity(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    limit: u32,
) -> Result<atlas_core::plugin_host::PluginActivityLog, AtlasError> {
    local_desktop(&window)?;
    workspace.plugin_activity(limit)
}
/// The kill switch. Deliberately not behind `begin`: it must work while
/// other work is in flight.
#[tauri::command]
fn plugins_set_all_off(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    off: bool,
) -> Result<atlas_core::plugin_host::PluginsState, AtlasError> {
    local_desktop(&window)?;
    workspace.plugins_set_all_off(off)
}
#[tauri::command]
fn plugin_revoke_trusted(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    plugin_id: String,
) -> Result<atlas_core::plugin_host::PluginsState, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.plugin_revoke_trusted(&plugin_id)
}
/// The pack commands allowed on a server, with what they last observed.
#[tauri::command]
fn pack_state(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<atlas_core::pack::PackServerState, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.pack_state(&server_id)
}
/// What packs add to the map: badges and relationship kinds (T044).
#[tauri::command]
fn pack_map(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<atlas_core::pack::PackMapState, AtlasError> {
    local_desktop(&window)?;
    workspace.pack_map()
}
/// The pack applications the owner allowed on one server (T044).
#[tauri::command]
fn pack_apps(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<atlas_core::pack::PackAppsState, AtlasError> {
    local_desktop(&window)?;
    workspace.pack_apps(&server_id)
}
/// Runs one pack command now, if the owner's settings admit it (T043).
#[tauri::command]
async fn pack_run(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    plugin_id: String,
    contribution_id: String,
    parameters: std::collections::BTreeMap<String, String>,
) -> Result<atlas_core::pack::PackServerState, AtlasError> {
    local_desktop(&window)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.run_pack_command(&server_id, &plugin_id, &contribution_id, &parameters)
    })
    .await
    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}
/// Asks the core whether a plugin may use a capability on a server. This is
/// the call a plugin runtime makes before acting; a refusal is logged.
#[tauri::command]
fn plugin_authorize(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    plugin_id: String,
    server_id: ServerId,
    capability: atlas_core::extensions::Capability,
) -> Result<atlas_core::plugin_host::PluginDecision, AtlasError> {
    local_desktop(&window)?;
    let _operation = workspace.inner().begin()?;
    workspace.plugin_authorize(&plugin_id, &server_id, capability)
}
#[tauri::command]
fn extensions_state(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<atlas_core::extensions::ExtensionCatalogue, AtlasError> {
    local_desktop(&window)?;
    Ok(workspace.extensions())
}
#[tauri::command]
fn workspace_status(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<WorkspaceStatus, AtlasError> {
    local_desktop(&window)?;
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
async fn map_state(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<MapState, AtlasError> {
    local_desktop(&window)?;
    run_store(workspace.inner().clone(), move |w| {
        w.with_store(|s| s.snapshot())
    })
    .await
}
#[tauri::command]
async fn retry_workspace(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<MapState, AtlasError> {
    local_desktop(&window)?;
    run_store(workspace.inner().clone(), Workspace::retry).await
}
#[tauri::command]
async fn add_server(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    input: AddServer,
) -> Result<MapState, AtlasError> {
    local_desktop(&window)?;
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
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    id: ServerId,
    input: UpdateServer,
) -> Result<MapState, AtlasError> {
    local_desktop(&window)?;
    run_store(workspace.inner().clone(), move |w| {
        let host_changed = input.host.is_some();
        let _contact = host_changed.then(|| w.live_contact_guard()).transpose()?;
        let (state, replaced) = w.with_store(|s| {
            let replaced = s.update_server_replacing(&id, input)?;
            Ok((s.snapshot()?, replaced))
        })?;
        w.invalidate_server(&id);
        if host_changed {
            w.cancel_health(&id)?;
        }
        // A discovered candidate that gave up its host is gone: end anything
        // still attached to it, exactly as removing a server does.
        if let Some(replaced) = &replaced {
            w.invalidate_server(replaced);
            w.cancel_health(replaced)?;
        }
        Ok(state)
    })
    .await
}
#[tauri::command]
async fn remove_server(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    id: ServerId,
) -> Result<MapState, AtlasError> {
    local_desktop(&window)?;
    run_store(workspace.inner().clone(), move |w| {
        let _contact = w.live_contact_guard()?;
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
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    input: AddRelationship,
) -> Result<MapState, AtlasError> {
    local_desktop(&window)?;
    run_store(workspace.inner().clone(), move |w| {
        w.add_relationship(input)
    })
    .await
}
#[tauri::command]
async fn remove_relationship(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
    id: EdgeId,
) -> Result<MapState, AtlasError> {
    local_desktop(&window)?;
    run_store(workspace.inner().clone(), move |w| {
        w.with_store(|s| {
            s.remove_relationship(&id)?;
            s.snapshot()
        })
    })
    .await
}
#[tauri::command]
async fn import_reports(
    window: tauri::Webview,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<MapState, AtlasError> {
    local_desktop(&window)?;
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
                workspace.tick_live();
                for session in workspace.reap_files() { let _ = handle.emit_to(main_webview(), "atlas-session-closed", session); }
                apps::reap(&handle, &workspace);
            }
        });
        // A plugin view's request reaches only InfraWerf's own interface.
        let views_handle = app.handle().clone();
        workspace.set_view_emitter(Box::new(move |request| {
            let _ = views_handle.emit_to(main_webview(), "atlas-plugin-action", request);
        }));
        app.manage(workspace);
        Ok(())
    })
    .on_window_event(|window, event| {
        if let tauri::WindowEvent::Destroyed = event {
            apps::window_destroyed(window);
        }
        // Only the main window closes the workspace; remote application
        // windows close freely and end just their own tunnel.
        if window.label() != "main" {
            return;
        }
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if !window.state::<Arc<Workspace>>().request_close() {
                api.prevent_close();
                let _ = window.emit_to(main_webview(), "atlas-close-blocked", "InfraWerf is finishing a local operation or has an open SSH session. Lock open sessions and wait for work to finish, then close again.");
            }
        }
    })
    .invoke_handler(tauri::generate_handler![extensions_state, plugins_state, plugin_enable, plugin_disable, plugin_install, plugin_update_check, plugin_update_apply, plugin_update_discard, plugin_remove, plugin_pick_source, plugin_set_grant, plugin_authorize, plugin_activity, plugins_set_all_off, plugin_revoke_trusted, pack_state, pack_run, plugin_views, plugin_view_open, plugin_view_bounds, plugin_view_close, plugin_views_open, plugin_view_answer, pack_map, pack_apps, apps::pack_app_open, apps::tunnel_open, apps::tunnel_discover, apps::tunnel_close, apps::tunnel_close_all, apps::tunnel_list, apps::tunnel_focus, monitoring_accept_identity, monitoring_state, monitoring_configure, monitoring_history_limit, monitoring_discover, foundation_status, workspace_status, map_state, retry_workspace, add_server, update_server, remove_server, add_relationship, remove_relationship, import_reports, ssh_prepare, ssh_verify, ssh_cancel, health_configure, health_status, health_cancel, inventory_refresh, files_prepare, files_open, files_read, workload_inspect, workload_control_preview, workload_control_confirm, files_lock, files_terminal_start, files_terminal_poll, files_terminal_write, files_terminal_resize, files_preview, files_commit, files_discard, files_release, files_pick_upload, files_pick_download, files_selection_discard, files_transfer, files_stage_protected, transfer_list, transfer_cancel, elevation_preview, elevation_confirm, elevation_cancel, protected_placement_preview, protected_placement_confirm, protected_placement_cancel])
    .run(tauri::generate_context!())
    .expect("InfraWerf native runtime failed");
}
