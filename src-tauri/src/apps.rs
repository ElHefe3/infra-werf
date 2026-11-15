//! Remote application windows (F17/T035).
//!
//! Each tunnel gets its own top-level window labelled `app-<tunnel id>`. The
//! window loads only the loopback tunnel origin, runs InPrivate (no cookies
//! or storage survive it), has no capability grant and fails the main-window
//! check in `local_desktop`, so remote pages cannot invoke InfraWerf commands.
use crate::{Workspace, local_desktop};
use atlas_core::{
    tunnel::{AppDiscovery, TunnelInfo, TunnelRequest},
    *,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

pub const PREFIX: &str = "app-";
pub const CHANGED: &str = "atlas-tunnels-changed";

fn label(tunnel_id: &str) -> String {
    format!("{PREFIX}{tunnel_id}")
}

fn open_window(app: &AppHandle, info: &TunnelInfo) -> Result<(), AtlasError> {
    let url: tauri::Url = info
        .url
        .parse()
        .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?;
    let origin = url.origin();
    WebviewWindowBuilder::new(app, label(&info.tunnel_id), WebviewUrl::External(url))
        .title(format!(
            "{} · localhost:{} — InfraWerf",
            info.server_name, info.remote_port
        ))
        .inner_size(1200.0, 800.0)
        .min_inner_size(480.0, 360.0)
        .incognito(true)
        // Stay on the tunnel origin; other destinations would leave the
        // reviewed forward (and could be any site).
        .on_navigation(move |target| target.origin() == origin)
        .build()
        .map(|_| ())
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))
}

pub fn close_window(app: &AppHandle, tunnel_id: &str) {
    if let Some(window) = app.get_webview_window(&label(tunnel_id)) {
        let _ = window.destroy();
    }
}

#[tauri::command]
pub async fn tunnel_open(
    window: tauri::WebviewWindow,
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
    request: TunnelRequest,
) -> Result<TunnelInfo, AtlasError> {
    local_desktop(&window)?;
    workspace.require_extension(atlas_core::extensions::REMOTE_APPS_OWNER)?;
    let workspace = workspace.inner().clone();
    let opened = workspace.clone();
    let info =
        tauri::async_runtime::spawn_blocking(move || opened.open_tunnel(&server_id, request))
            .await
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))??;
    if let Err(error) = open_window(&app, &info) {
        let _ = workspace.close_tunnel(&info.tunnel_id);
        return Err(error);
    }
    let _ = app.emit_to("main", CHANGED, ());
    Ok(info)
}

#[tauri::command]
pub async fn tunnel_discover(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    server_id: ServerId,
) -> Result<AppDiscovery, AtlasError> {
    local_desktop(&window)?;
    workspace.require_extension(atlas_core::extensions::REMOTE_APPS_OWNER)?;
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || workspace.discover_applications(&server_id))
        .await
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
}

#[tauri::command]
pub fn tunnel_close(
    window: tauri::WebviewWindow,
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    tunnel_id: String,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.require_extension(atlas_core::extensions::REMOTE_APPS_OWNER)?;
    workspace.close_tunnel(&tunnel_id)?;
    close_window(&app, &tunnel_id);
    let _ = app.emit_to("main", CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn tunnel_close_all(
    window: tauri::WebviewWindow,
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.require_extension(atlas_core::extensions::REMOTE_APPS_OWNER)?;
    for id in workspace.close_all_tunnels() {
        close_window(&app, &id);
    }
    let _ = app.emit_to("main", CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn tunnel_list(
    window: tauri::WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<Vec<TunnelInfo>, AtlasError> {
    local_desktop(&window)?;
    workspace.require_extension(atlas_core::extensions::REMOTE_APPS_OWNER)?;
    workspace.tunnels()
}

#[tauri::command]
pub fn tunnel_focus(
    window: tauri::WebviewWindow,
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    tunnel_id: String,
) -> Result<(), AtlasError> {
    local_desktop(&window)?;
    workspace.require_extension(atlas_core::extensions::REMOTE_APPS_OWNER)?;
    let target = app
        .get_webview_window(&label(&tunnel_id))
        .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
    let _ = target.unminimize();
    let _ = target.show();
    target
        .set_focus()
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))
}

/// Window lifecycle: a destroyed app window ends its tunnel. The main window
/// is InfraWerf itself: once its close was accepted, end every tunnel and exit
/// even while application windows are still open (otherwise they would keep
/// the process alive without the workspace).
pub fn window_destroyed(window: &tauri::Window) {
    let app = window.app_handle();
    let Some(workspace) = app.try_state::<Arc<Workspace>>() else {
        return;
    };
    if let Some(id) = window.label().strip_prefix(PREFIX) {
        let _ = workspace.close_tunnel(id);
        let _ = app.emit_to("main", CHANGED, ());
    } else if window.label() == "main" {
        // Dropping the entries kills and reaps each OpenSSH child.
        let _ = workspace.close_all_tunnels();
        app.exit(0);
    }
}

/// Periodic reaping from the background tick.
pub fn reap(app: &AppHandle, workspace: &Workspace) {
    let ended = workspace.reap_tunnels();
    for id in &ended {
        close_window(app, id);
    }
    if !ended.is_empty() {
        let _ = app.emit_to("main", CHANGED, ());
    }
}
