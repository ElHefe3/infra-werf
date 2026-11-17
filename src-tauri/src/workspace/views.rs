//! Plugin views owned by the workspace (F23/T047).
//!
//! A view is a child webview of the main window showing a page served by
//! `atlas_core::plugin_view::ViewServer` on its own loopback origin. Tauri
//! treats that origin as remote, so the page cannot call InfraWerf commands,
//! and every command also checks that its caller is the main webview. The
//! page can only use the view message API below, and each call is decided
//! against the owner's grants at that moment. An action a view asks for is
//! shown to the owner in InfraWerf's own dialog and runs only if the owner
//! presses Run there. Views close when the owner closes or locks them, when
//! the workspace closes, and whenever what plugins may do changes.
use super::*;
use atlas_core::extensions::Capability;
use atlas_core::plugin_host::{
    ACTION_OPEN_VIEW, ACTION_VIEW_BLOCKED, ACTION_VIEW_REQUEST, ActivityOutcome, PluginViewTab,
    PluginViewsState,
};
use atlas_core::plugin_view::{
    ApiReply, VIEW_API_VERSION, ViewActionRequest, ViewActionState, ViewApi, ViewServer,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Weak;
use std::sync::atomic::{AtomicU64, Ordering};

/// Log rows one opening of a view may cause, whatever their kind: blocked
/// attempts, refused requests and refused reads. Further ones are counted
/// and summed up in one row when the view closes, so a page cannot flood
/// the activity log and push other entries out (T047 review, findings 2, 5).
const LOG_BUDGET: u32 = 40;

/// The guard's report words; anything else is logged as `other`.
const GUARD_WORDS: &[&str] = &[
    "chrome.webview",
    "__TAURI__",
    "__TAURI_IPC__",
    "ipc",
    "__TAURI_POST_MESSAGE__",
    "RTCPeerConnection",
    "webkitRTCPeerConnection",
    "RTCDataChannel",
    "RTCIceTransport",
    "RTCSctpTransport",
    "RTCDtlsTransport",
    "RTCRtpSender",
    "RTCRtpReceiver",
];

/// The script every view runs before its own: it removes the webview's
/// native message channel and Tauri's internals from the page, and reports
/// any later attempt to use them. Isolation does not rely on it: Tauri
/// refuses IPC from this origin and every command checks its caller.
const VIEW_GUARD: &str = r#"(() => {
  const report = what => { try { fetch(location.pathname.split('/').slice(0, 2).join('/') + '/infrawerf/v1/blocked', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ what }) }); } catch (_) {} };
  const trap = name => ({ get() { report(name); return undefined; }, set() { report(name); }, configurable: false });
  try { if (window.chrome && window.chrome.webview) { delete window.chrome.webview; } } catch (_) {}
  try { Object.defineProperty(window.chrome || {}, 'webview', trap('chrome.webview')); } catch (_) {}
  // Tauri's own internals are frozen and cannot be removed. Their two
  // routes are closed: the native channel is removed here, and the content
  // security policy blocks requests to ipc.localhost (and reports them).
  // WebRTC is not covered by the content security policy; it is also off
  // in the browser. Remove it here as a second layer.
  for (const name of ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel', 'RTCIceTransport', 'RTCSctpTransport', 'RTCDtlsTransport', 'RTCRtpSender', 'RTCRtpReceiver']) {
    try { delete window[name]; } catch (_) {}
    try { Object.defineProperty(window, name, trap(name)); } catch (_) {}
  }
  for (const name of ['__TAURI__', '__TAURI_IPC__', 'ipc', '__TAURI_POST_MESSAGE__']) {
    try { delete window[name]; } catch (_) {}
    try { Object.defineProperty(window, name, trap(name)); } catch (_) {}
  }
})();"#;

struct PendingAction {
    request: ViewActionRequest,
    /// Parameter name to workload ID, as the run needs it.
    workload_ids: BTreeMap<String, String>,
    state: ViewActionState,
    observation: Option<atlas_core::pack::PackObservation>,
}

struct OpenView {
    tab: PluginViewTab,
    server: ServerId,
    webview: tauri::Webview,
    link: Arc<ViewLink>,
    _server: ViewServer,
}

#[derive(Default)]
pub(super) struct Views {
    open: Mutex<HashMap<String, OpenView>>,
    actions: Mutex<HashMap<String, PendingAction>>,
    next: AtomicU64,
}

/// The rectangle of the tab area, in logical pixels of the main window.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ViewBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ViewBounds {
    fn rect(self) -> tauri::Rect {
        tauri::Rect {
            position: tauri::LogicalPosition::new(self.x.max(0.0), self.y.max(0.0)).into(),
            size: tauri::LogicalSize::new(self.width.max(1.0), self.height.max(1.0)).into(),
        }
    }
}

/// One view's link to InfraWerf.
struct ViewLink {
    workspace: Weak<Workspace>,
    label: String,
    tab: PluginViewTab,
    server: ServerId,
    /// Rows this opening may still write.
    budget: std::sync::atomic::AtomicU32,
    /// Rows not written because the budget ran out.
    unlogged: std::sync::atomic::AtomicU32,
    /// Requests refused before they reached the view: no secret, wrong host
    /// or another origin. Anything on this PC can send those, so they are
    /// not charged to the plugin; one row counts them at close.
    outside: std::sync::atomic::AtomicU32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionAsk {
    contribution_id: String,
    #[serde(default)]
    parameters: BTreeMap<String, String>,
}

impl ViewLink {
    /// Logs one row within the opening's budget.
    fn note(&self, workspace: &Workspace, action: &str, outcome: ActivityOutcome, detail: &str) {
        let spent = self
            .budget
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            });
        if spent.is_ok() {
            self.log(workspace, action, outcome, detail);
        } else {
            self.unlogged.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// The rows written when the view closes, outside the budget.
    fn close_note(&self, workspace: &Workspace) {
        let unlogged = self.unlogged.swap(0, Ordering::SeqCst);
        if unlogged > 0 {
            self.log(
                workspace,
                ACTION_VIEW_BLOCKED,
                ActivityOutcome::Refused,
                &format!("{unlogged} more refused or blocked attempts were not logged one by one"),
            );
        }
        let outside = self.outside.swap(0, Ordering::SeqCst);
        if outside > 0 {
            self.log(workspace, ACTION_VIEW_BLOCKED, ActivityOutcome::Refused, &format!("{outside} requests from outside the view were refused (not counted as the plugin's)"));
        }
    }

    fn log(&self, workspace: &Workspace, action: &str, outcome: ActivityOutcome, detail: &str) {
        let directory = workspace.plugins_directory();
        let _ = workspace.with_store(|store| {
            store.plugin_view_event(
                &directory,
                &self.tab.plugin_id,
                &self.server,
                action,
                outcome,
                detail,
            )
        });
    }

    /// Admits one capability now. A refusal is logged within the budget;
    /// an allowed read is not logged on every call, as a page may poll.
    fn allowed(&self, workspace: &Workspace, capability: Capability) -> bool {
        let directory = workspace.plugins_directory();
        match workspace.with_store(|store| {
            store.plugin_check(&directory, &self.tab.plugin_id, &self.server, capability)
        }) {
            Ok(Ok(())) => true,
            Ok(Err(refusal)) => {
                let capability = serde_json::to_value(capability)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let refusal = serde_json::to_value(refusal)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                self.note(workspace, &capability, ActivityOutcome::Refused, &refusal);
                false
            }
            Err(_) => false,
        }
    }

    fn still_open(&self, workspace: &Workspace) -> bool {
        workspace
            .views
            .open
            .lock()
            .is_ok_and(|open| open.contains_key(&self.label))
    }

    fn not_granted() -> ApiReply {
        ApiReply::refused(
            403,
            "not_granted",
            "The owner has not allowed this plugin to do that on this server.",
        )
    }

    fn context(&self, workspace: &Workspace) -> ApiReply {
        // The server's name and host are the owner's saved data: only with
        // read_saved_state (T047 review, finding 1).
        if !self.allowed(workspace, Capability::ReadSavedState) {
            return ApiReply::ok(serde_json::json!({
                "api_version": VIEW_API_VERSION,
                "plugin_id": self.tab.plugin_id,
                "contribution_id": self.tab.contribution_id,
                "server": null,
            }));
        }
        let server = workspace
            .with_store(|store| store.snapshot())
            .ok()
            .and_then(|state| state.servers.into_iter().find(|s| s.id == self.server));
        let Some(server) = server else {
            return ApiReply::refused(404, "unknown_server", "The server is no longer on the map.");
        };
        ApiReply::ok(serde_json::json!({
            "api_version": VIEW_API_VERSION,
            "plugin_id": self.tab.plugin_id,
            "contribution_id": self.tab.contribution_id,
            "server": { "id": server.id, "name": server.name, "host": server.host },
        }))
    }

    fn observations(&self, workspace: &Workspace) -> ApiReply {
        if !self.allowed(workspace, Capability::ReadSavedState) {
            return Self::not_granted();
        }
        match workspace.pack_state(&self.server) {
            Ok(state) => ApiReply::ok(
                state
                    .commands
                    .into_iter()
                    .filter(|c| c.plugin_id == self.tab.plugin_id)
                    .map(|c| {
                        serde_json::json!({
                            "contribution_id": c.contribution_id,
                            "title": c.title,
                            "health_check": c.health_check,
                            "parameters": c.parameters.iter().map(|p| serde_json::json!({
                                "name": p.name,
                                "source": p.source,
                                "choices": p.choices.iter().map(|choice| serde_json::json!({ "id": choice.id, "name": choice.name })).collect::<Vec<_>>(),
                            })).collect::<Vec<_>>(),
                            "observation": c.observation,
                        })
                    })
                    .collect::<Vec<_>>(),
            ),
            Err(_) => ApiReply::refused(500, "unavailable", "InfraWerf could not read the observations."),
        }
    }

    fn inventory(&self, workspace: &Workspace) -> ApiReply {
        if !self.allowed(workspace, Capability::ReadSavedState) {
            return Self::not_granted();
        }
        let server = workspace
            .with_store(|store| store.snapshot())
            .ok()
            .and_then(|state| state.servers.into_iter().find(|s| s.id == self.server));
        match server {
            Some(server) => ApiReply::ok(serde_json::json!({ "inventory": server.inventory })),
            None => ApiReply::refused(404, "unknown_server", "The server is no longer on the map."),
        }
    }

    fn ask(&self, workspace: &Arc<Workspace>, body: &[u8]) -> ApiReply {
        let Ok(ask) = serde_json::from_slice::<ActionAsk>(body) else {
            return ApiReply::refused(
                400,
                "bad_request",
                "Send {\"contribution_id\": ..., \"parameters\": {...}}.",
            );
        };
        let directory = workspace.plugins_directory();
        // Only a command of this plugin that the owner allowed here now.
        let run = workspace
            .with_store(|store| store.pack_commands(&directory, &self.server))
            .ok()
            .and_then(|runs| {
                runs.into_iter().find(|r| {
                    r.plugin_id == self.tab.plugin_id && r.contribution.id == ask.contribution_id
                })
            });
        let Some(run) = run else {
            // The page's own text is logged only if it names a real
            // contribution of this plugin (T047 review, finding 2).
            let real = workspace
                .plugins_state()
                .ok()
                .and_then(|state| {
                    state
                        .plugins
                        .into_iter()
                        .find(|p| p.id == self.tab.plugin_id)
                })
                .is_some_and(|plugin| {
                    plugin
                        .contributions
                        .iter()
                        .any(|c| c.id == ask.contribution_id)
                });
            let detail = if real {
                ask.contribution_id.as_str()
            } else {
                "unknown_contribution"
            };
            self.note(
                workspace,
                ACTION_VIEW_REQUEST,
                ActivityOutcome::Refused,
                detail,
            );
            return Self::not_granted();
        };
        let declared: Vec<&str> = run
            .command
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        if ask.parameters.len() != declared.len()
            || !ask
                .parameters
                .keys()
                .all(|k| declared.contains(&k.as_str()))
        {
            return ApiReply::refused(
                400,
                "bad_parameters",
                "Give exactly the command's parameters, each as a workload ID from the observations.",
            );
        }
        // Each value must be a workload this server's saved inventory offers.
        let Ok(state) = workspace.pack_state(&self.server) else {
            return ApiReply::refused(500, "unavailable", "InfraWerf could not read the server.");
        };
        let view = state.commands.iter().find(|c| {
            c.plugin_id == self.tab.plugin_id && c.contribution_id == ask.contribution_id
        });
        let mut names = BTreeMap::new();
        for (name, id) in &ask.parameters {
            let choice = view
                .and_then(|v| v.parameters.iter().find(|p| &p.name == name))
                .and_then(|p| p.choices.iter().find(|c| &c.id == id));
            match choice {
                Some(choice) => {
                    names.insert(name.clone(), choice.name.clone());
                }
                None => {
                    self.note(
                        workspace,
                        ACTION_VIEW_REQUEST,
                        ActivityOutcome::Refused,
                        &ask.contribution_id,
                    );
                    return ApiReply::refused(
                        403,
                        "unknown_workload",
                        "A parameter is not a workload from this server's inventory.",
                    );
                }
            }
        }
        let mut actions = match workspace.views.actions.lock() {
            Ok(actions) => actions,
            Err(_) => {
                return ApiReply::refused(
                    500,
                    "unavailable",
                    "InfraWerf could not take the request.",
                );
            }
        };
        if actions.values().any(|a| {
            a.request.view == self.label
                && matches!(a.state, ViewActionState::Waiting | ViewActionState::Running)
        }) {
            return ApiReply::refused(
                409,
                "busy",
                "This view already has a request waiting for the owner.",
            );
        }
        let id = OperationId::generate().as_str().to_owned();
        let server_name = state_server_name(workspace, &self.server);
        let request = ViewActionRequest {
            id: id.clone(),
            view: self.label.clone(),
            plugin_id: self.tab.plugin_id.clone(),
            plugin_name: self.tab.plugin_name.clone(),
            server_id: self.server.clone(),
            server_name,
            contribution_id: ask.contribution_id.clone(),
            title: run.contribution.title.clone(),
            argv: run.command.argv.clone(),
            parameters: names,
        };
        actions.insert(
            id.clone(),
            PendingAction {
                request: request.clone(),
                workload_ids: ask.parameters,
                state: ViewActionState::Waiting,
                observation: None,
            },
        );
        drop(actions);
        // A request that reaches the owner is always logged, outside the
        // budget: there is at most one waiting per view, and the owner's own
        // answer ends it (re-test, finding 15).
        self.log(
            workspace,
            ACTION_VIEW_REQUEST,
            ActivityOutcome::NeedsConfirmation,
            &ask.contribution_id,
        );
        if let Some(emit) = workspace.views_emitter() {
            emit(request);
        }
        ApiReply::ok(serde_json::json!({ "id": id, "state": ViewActionState::Waiting }))
    }

    fn action(&self, workspace: &Workspace, id: &str) -> ApiReply {
        let actions = workspace.views.actions.lock();
        match actions.ok().and_then(|a| {
            a.get(id)
                .filter(|a| a.request.view == self.label)
                .map(|a| (a.state.clone(), a.observation.clone()))
        }) {
            Some((state, observation)) => ApiReply::ok(
                serde_json::json!({ "id": id, "state": state, "observation": observation }),
            ),
            None => ApiReply::refused(404, "unknown_action", "No such request from this view."),
        }
    }
}

fn state_server_name(workspace: &Workspace, server: &ServerId) -> String {
    workspace
        .with_store(|store| store.snapshot())
        .ok()
        .and_then(|state| state.servers.into_iter().find(|s| &s.id == server))
        .map(|s| s.name)
        .unwrap_or_default()
}

impl ViewApi for ViewLink {
    fn call(&self, method: &str, path: &str, body: &[u8]) -> ApiReply {
        let Some(workspace) = self.workspace.upgrade() else {
            return ApiReply::refused(410, "closed", "InfraWerf is closing.");
        };
        if !self.still_open(&workspace) {
            return ApiReply::refused(410, "closed", "This view was closed.");
        }
        match (method, path) {
            ("GET", "context") => self.context(&workspace),
            ("GET", "observations") => self.observations(&workspace),
            ("GET", "inventory") => self.inventory(&workspace),
            ("POST", "actions") => self.ask(&workspace, body),
            ("GET", action) if action.starts_with("actions/") => {
                self.action(&workspace, &action["actions/".len()..])
            }
            ("POST", "blocked") => {
                let what = serde_json::from_slice::<serde_json::Value>(body)
                    .ok()
                    .and_then(|v| v.get("what").and_then(|w| w.as_str()).map(str::to_owned))
                    .unwrap_or_default();
                let what = GUARD_WORDS
                    .iter()
                    .find(|w| **w == what)
                    .copied()
                    .unwrap_or("other");
                self.blocked("ipc", what);
                ApiReply::ok(serde_json::json!({}))
            }
            _ => {
                self.blocked(
                    "unknown_call",
                    &format!("{method} {}", path.chars().take(60).collect::<String>()),
                );
                ApiReply::refused(
                    404,
                    "unknown_call",
                    "The view message API v1 has no such call.",
                )
            }
        }
    }

    fn blocked(&self, what: &str, detail: &str) {
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        if matches!(
            what,
            "no_secret" | "wrong_host" | "cross_origin" | "busy" | "bad_request"
        ) {
            self.outside.fetch_add(1, Ordering::SeqCst);
            return;
        }
        let detail = if detail.is_empty() {
            what.to_owned()
        } else {
            format!("{what}: {detail}")
        };
        self.note(
            &workspace,
            ACTION_VIEW_BLOCKED,
            ActivityOutcome::Refused,
            &detail,
        );
    }
}

type Emitter = Box<dyn Fn(ViewActionRequest) + Send + Sync>;
static EMITTER: std::sync::OnceLock<Emitter> = std::sync::OnceLock::new();

impl Workspace {
    /// Set once at start-up: how a view's request reaches the main window.
    pub fn set_view_emitter(&self, emit: Emitter) {
        let _ = EMITTER.set(emit);
    }
    fn views_emitter(&self) -> Option<&'static Emitter> {
        EMITTER.get()
    }

    pub fn plugin_views(&self, server: &ServerId) -> Result<PluginViewsState, AtlasError> {
        let directory = self.plugins_directory();
        Ok(PluginViewsState {
            contract_version: CONTRACT_VERSION,
            server_id: server.clone(),
            views: self
                .with_store(|store| store.plugin_views(&directory, server))?
                .into_iter()
                .map(|source| source.tab)
                .collect(),
        })
    }

    /// Opens a plugin view over the tab area of the main window. Returns
    /// its label, used to move, hide and close it.
    pub fn open_plugin_view(
        self: &Arc<Self>,
        main: &tauri::Window,
        server: &ServerId,
        plugin: &str,
        contribution: &str,
        bounds: ViewBounds,
    ) -> Result<String, AtlasError> {
        let directory = self.plugins_directory();
        let source = self
            .with_store(|store| store.plugin_views(&directory, server))?
            .into_iter()
            .find(|s| s.tab.plugin_id == plugin && s.tab.contribution_id == contribution)
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        let label = format!(
            "pluginview-{}",
            self.views.next.fetch_add(1, Ordering::SeqCst)
        );
        let link = Arc::new(ViewLink {
            workspace: Arc::downgrade(self),
            label: label.clone(),
            tab: source.tab.clone(),
            server: server.clone(),
            budget: std::sync::atomic::AtomicU32::new(LOG_BUDGET),
            unlogged: std::sync::atomic::AtomicU32::new(0),
            outside: std::sync::atomic::AtomicU32::new(0),
        });
        // The view serves what it reads now, with links refused, and only
        // if that is what was installed (T047 review, findings 7 and 8).
        let files = atlas_core::plugin_install::read_folder(&source.folder)
            .map_err(|_| AtlasError::new(ErrorCode::Denied))?;
        if source
            .content_sha256
            .as_deref()
            .is_some_and(|recorded| recorded != atlas_core::plugin_install::content_hash(&files))
        {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let files: atlas_core::plugin_view::ViewFiles = files.into_iter().collect();
        let view_server = ViewServer::start(files, &source.entry, link.clone())
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
        let url: tauri::Url = view_server
            .url()
            .parse()
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
        let origin = url.origin();
        let (watch, navigation) = (Arc::downgrade(&link), Arc::downgrade(&link));
        let builder = tauri::webview::WebviewBuilder::new(&label, tauri::WebviewUrl::External(url))
            .incognito(true)
            .devtools(false)
            .disable_drag_drop_handler()
            .initialization_script_for_all_frames(VIEW_GUARD)
            // The page stays on its own origin; anything else is refused
            // and logged.
            .on_navigation(move |target| {
                // Only plain http on the view's own host and port: no blob:,
                // data: or about: documents, where the guard does not run.
                let same = target.scheme() == "http" && target.origin() == origin;
                if !same && let Some(link) = navigation.upgrade() {
                    link.blocked(
                        "navigation",
                        &format!("{}://{}", target.scheme(), target.host_str().unwrap_or("")),
                    );
                }
                same
            })
            .on_new_window(move |target, _| {
                if let Some(link) = watch.upgrade() {
                    link.blocked(
                        "new_window",
                        &format!("{}://{}", target.scheme(), target.host_str().unwrap_or("")),
                    );
                }
                tauri::webview::NewWindowResponse::Deny
            })
            .on_download(|_, _| false);
        let rect = bounds.rect();
        // A view never opens while plugins may not act.
        let webview = main
            .add_child(builder, rect.position, rect.size)
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
        self.views.open.lock().map_err(|_| failure())?.insert(
            label.clone(),
            OpenView {
                tab: source.tab.clone(),
                server: server.clone(),
                webview,
                link: link.clone(),
                _server: view_server,
            },
        );
        link.log(self, ACTION_OPEN_VIEW, ActivityOutcome::Done, contribution);
        Ok(label)
    }

    pub fn plugin_view_bounds(
        &self,
        label: &str,
        bounds: Option<ViewBounds>,
    ) -> Result<(), AtlasError> {
        let open = self.views.open.lock().map_err(|_| failure())?;
        let view = open
            .get(label)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let done = match bounds {
            Some(bounds) => view
                .webview
                .set_bounds(bounds.rect())
                .and_then(|_| view.webview.show()),
            None => view.webview.hide(),
        };
        done.map_err(|_| AtlasError::new(ErrorCode::IoFailure))
    }

    /// Closes one view. Its server stops and its waiting requests are
    /// cancelled.
    pub fn close_plugin_view(&self, label: &str) {
        let removed = self
            .views
            .open
            .lock()
            .ok()
            .and_then(|mut open| open.remove(label));
        if let Some(view) = removed {
            let _ = view.webview.close();
            view.link.close_note(self);
        }
        if let Ok(mut actions) = self.views.actions.lock() {
            for action in actions.values_mut() {
                if action.request.view == label && action.state == ViewActionState::Waiting {
                    action.state = ViewActionState::Cancelled;
                }
            }
        }
    }

    /// Closes every view; called whenever what plugins may do changes.
    pub(super) fn close_plugin_views(&self) {
        let labels: Vec<String> = self
            .views
            .open
            .lock()
            .map(|open| open.keys().cloned().collect())
            .unwrap_or_default();
        for label in labels {
            self.close_plugin_view(&label);
        }
    }

    /// The views open now: label, plugin, contribution and server.
    pub fn open_plugin_views(&self) -> Vec<serde_json::Value> {
        self.views
            .open
            .lock()
            .map(|open| {
                open.iter()
                    .map(|(label, view)| {
                        // Where it really is, in the main window's logical pixels.
                        let scale = view.webview.window().scale_factor().unwrap_or(1.0);
                        let bounds = view.webview.bounds().ok().map(|rect| {
                            let position = rect.position.to_logical::<f64>(scale);
                            let size = rect.size.to_logical::<f64>(scale);
                            serde_json::json!({ "x": position.x, "y": position.y, "width": size.width, "height": size.height })
                        });
                        serde_json::json!({
                            "bounds": bounds,
                            "label": label,
                            "plugin_id": view.tab.plugin_id,
                            "contribution_id": view.tab.contribution_id,
                            "server_id": view.server,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The owner's answer to a view's request, from InfraWerf's own dialog.
    /// Run means run it now, through the same checks as pressing Run in the
    /// Plugin data tab.
    pub fn answer_view_action(
        self: &Arc<Self>,
        id: &str,
        run: bool,
    ) -> Result<ViewActionState, AtlasError> {
        let (request, workload_ids) = {
            let mut actions = self.views.actions.lock().map_err(|_| failure())?;
            let action = actions
                .get_mut(id)
                .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
            if action.state != ViewActionState::Waiting {
                return Ok(action.state.clone());
            }
            if !run {
                action.state = ViewActionState::Declined;
                let request = action.request.clone();
                drop(actions);
                let directory = self.plugins_directory();
                let _ = self.with_store(|store| {
                    store.plugin_view_event(
                        &directory,
                        &request.plugin_id,
                        &request.server_id,
                        ACTION_VIEW_REQUEST,
                        ActivityOutcome::Refused,
                        &format!("{}: declined by you", request.contribution_id),
                    )
                });
                return Ok(ViewActionState::Declined);
            }
            // Marked under the lock, so the same request cannot run twice
            // (T047 review, finding 9).
            action.state = ViewActionState::Running;
            (action.request.clone(), action.workload_ids.clone())
        };
        let still_open = self
            .views
            .open
            .lock()
            .map_err(|_| failure())?
            .contains_key(&request.view);
        let outcome = if still_open {
            self.run_pack_command(
                &request.server_id,
                &request.plugin_id,
                &request.contribution_id,
                &workload_ids,
            )
        } else {
            Err(AtlasError::new(ErrorCode::Cancelled))
        };
        let mut actions = self.views.actions.lock().map_err(|_| failure())?;
        let action = actions
            .get_mut(id)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        match outcome {
            Ok(state) => {
                action.observation = state
                    .commands
                    .into_iter()
                    .find(|c| {
                        c.plugin_id == request.plugin_id
                            && c.contribution_id == request.contribution_id
                    })
                    .and_then(|c| c.observation);
                action.state = ViewActionState::Done;
            }
            Err(error) if error.code == ErrorCode::Cancelled => {
                action.state = ViewActionState::Cancelled
            }
            Err(_) => action.state = ViewActionState::Failed,
        }
        Ok(action.state.clone())
    }
}
