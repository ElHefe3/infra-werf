use atlas_core::files::{
    ActionPassword, ElevationPreview, ElevationReceipt, LocalSelection, LocalTransfer,
    ProtectedPlacementReceipt, ProtectedPlacementRequest, ProtectedPlacementStage,
    TransferDirection, TransferView,
};
use atlas_core::*;
mod monitoring;
mod tunnels;
use atlas_core::{
    files::{
        FileAction, FileChange, FileMutationPreview, FileMutationReceipt, FileResult, FileSession,
        FileSessions, SessionPolicy,
    },
    health::{HealthProbeConfig, HealthSnapshot, SharedSnapshot, backoff_seconds, probe_once},
    operations::{Authority, OperationBroker, OperationTicket, ServerTarget},
    ssh::{SshOptions, SshVerification},
    workloads::{
        WorkloadAction, WorkloadControlPreview, WorkloadControlReceipt, WorkloadInspection,
    },
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::AtomicBool,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
struct LocalGrant {
    value: LocalTransfer,
    target: ServerTarget,
    session: SessionId,
    expires: std::time::Instant,
}

fn failure() -> AtlasError {
    AtlasError::new(ErrorCode::IoFailure)
}

#[derive(Clone)]
pub struct Paths {
    pub database: PathBuf,
    pub legacy: Option<PathBuf>,
    pub reports: Option<PathBuf>,
}

impl Paths {
    pub fn from_directory(directory: PathBuf) -> Self {
        Self {
            database: std::env::var_os("ATLAS_NATIVE_DB")
                .map(PathBuf::from)
                .unwrap_or_else(|| directory.join("atlas.sqlite3")),
            legacy: std::env::var_os("ATLAS_LEGACY_DB").map(PathBuf::from),
            reports: std::env::var_os("ATLAS_REPORTS_DIR").map(PathBuf::from),
        }
    }

    fn open(&self) -> Result<Store, AtlasError> {
        if self.database.exists() {
            return Store::open(&self.database);
        }
        if let Some(source) = &self.legacy {
            // Read-only migration creates a separate native file and retains
            // a private recovery copy. Never fall back to empty on failure.
            let recovery = self.database.with_file_name(format!(
                "legacy-recovery-{}.sqlite3",
                ServerId::generate().as_str()
            ));
            migrate_legacy(source, &self.database, &recovery)?;
            Store::open(&self.database)
        } else {
            Store::create(&self.database)
        }
    }
}

#[derive(Default)]
struct Lifecycle {
    active: u32,
    closing: bool,
}

struct HealthTask {
    config: HealthProbeConfig,
    snapshot: SharedSnapshot,
    cancel: Arc<AtomicBool>,
    generation: u64,
    due: Instant,
    active: bool,
    failures: u8,
}
#[derive(Default)]
struct HealthMonitor {
    tasks: HashMap<ServerId, HealthTask>,
    active: usize,
    generation: u64,
}

pub struct Workspace {
    paths: Paths,
    store: Mutex<Result<Store, AtlasError>>,
    lifecycle: Mutex<Lifecycle>,
    broker: OperationBroker,
    authority: Authority,
    ssh: SshOptions,
    files: FileSessions,
    local_grants: Mutex<HashMap<String, LocalGrant>>,
    health: Arc<Mutex<HealthMonitor>>,
    live: Mutex<monitoring::LiveScheduler>,
    live_contact: Mutex<()>,
    tunnels: Mutex<tunnels::Tunnels>,
}

impl Workspace {
    pub fn new(paths: Paths) -> Arc<Self> {
        let store = paths.open();
        let broker = OperationBroker::default();
        let authority = broker
            .issue_backend_authority()
            .expect("fresh local broker authority");
        let ssh = SshOptions::system();
        let files = FileSessions::new(
            broker.clone(),
            authority.clone(),
            ssh.clone(),
            SessionPolicy::default(),
        );
        Arc::new(Self {
            paths,
            store: Mutex::new(store),
            lifecycle: Mutex::new(Lifecycle::default()),
            broker,
            authority,
            ssh,
            files,
            local_grants: Mutex::new(HashMap::new()),
            health: Arc::new(Mutex::new(HealthMonitor::default())),
            live: Mutex::new(monitoring::LiveScheduler::default()),
            live_contact: Mutex::new(()),
            tunnels: Mutex::new(tunnels::Tunnels::default()),
        })
    }

    pub fn live_contact_guard(&self) -> Result<std::sync::MutexGuard<'_, ()>, AtlasError> {
        self.live_contact.lock().map_err(|_| failure())
    }
    /// Registered extension points for the interface (S12/F18).
    pub fn extensions(&self) -> atlas_core::extensions::ExtensionCatalogue {
        self.broker.extensions().catalogue()
    }
    pub fn status(&self) -> Result<WorkspaceStatus, AtlasError> {
        Ok(WorkspaceStatus {
            contract_version: CONTRACT_VERSION,
            storage_ready: self.store.lock().map_err(|_| failure())?.is_ok(),
            report_import_available: self.paths.reports.is_some(),
            coordinator_available: false, // F05 owns the authenticated listener.
            pending_operations: self.lifecycle.lock().map_err(|_| failure())?.active,
        })
    }

    pub fn begin(self: &Arc<Self>) -> Result<Operation, AtlasError> {
        let mut lifecycle = self.lifecycle.lock().map_err(|_| failure())?;
        if lifecycle.closing {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        lifecycle.active += 1;
        Ok(Operation(self.clone()))
    }

    /// The same lock admits operations and closes the app, avoiding a race
    /// between the close event and a queued persistence operation.
    pub fn request_close(&self) -> bool {
        self.cancel_all_health();
        let Ok(mut lifecycle) = self.lifecycle.lock() else {
            return false;
        };
        if lifecycle.active != 0 {
            return false;
        }
        lifecycle.closing = true;
        true
    }

    pub fn with_store<T>(
        &self,
        action: impl FnOnce(&mut Store) -> Result<T, AtlasError>,
    ) -> Result<T, AtlasError> {
        let mut lock = self.store.lock().map_err(|_| failure())?;
        match &mut *lock {
            Ok(store) => action(store),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn retry(&self) -> Result<MapState, AtlasError> {
        let mut lock = self.store.lock().map_err(|_| failure())?;
        if lock.is_err() {
            *lock = self.paths.open();
        }
        let snapshot = match &*lock {
            Ok(store) => store.snapshot(),
            Err(error) => Err(error.clone()),
        }?;
        drop(lock);
        self.apply_health(snapshot)
    }

    pub fn apply_health(&self, mut state: MapState) -> Result<MapState, AtlasError> {
        let records = self.with_store(|store| store.monitoring())?.records;
        let monitor = self.health.lock().map_err(|_| failure())?;
        for server in &mut state.servers {
            if let Some(task) = monitor.tasks.get(&server.id) {
                server.reachability = task.snapshot.get()?.host_reachability;
            }
            if let Some(record) = records
                .iter()
                .find(|r| r.server_id == server.id && r.host == server.host)
            {
                if record.policy.permits_contact() {
                    server.reachability = record.reachability.clone();
                }
            }
        }
        Ok(state)
    }

    pub fn configure_health(
        &self,
        id: &ServerId,
        config: HealthProbeConfig,
    ) -> Result<HealthSnapshot, AtlasError> {
        let _contact = self.live_contact_guard()?;
        config.validate()?;
        self.broker.extensions().authorize(
            atlas_core::extensions::health_check_id(config.service_kind),
            atlas_core::extensions::Capability::NetworkProbe,
        )?;
        if self
            .with_store(|s| s.monitoring())?
            .records
            .iter()
            .any(|r| {
                r.server_id == *id
                    && (r.policy.owner_offline
                        || r.policy.lifecycle != atlas_core::monitoring::ServerLifecycle::Active)
            })
        {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        if config.server_id != *id || self.ssh_target(id)?.host() != config.host {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let snapshot = SharedSnapshot::new(&config);
        let initial = snapshot.get()?;
        let mut monitor = self.health.lock().map_err(|_| failure())?;
        if let Some(previous) = monitor.tasks.remove(id) {
            previous
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
        monitor.generation = monitor.generation.wrapping_add(1).max(1);
        let generation = monitor.generation;
        monitor.tasks.insert(
            id.clone(),
            HealthTask {
                config,
                snapshot,
                cancel: Arc::new(AtomicBool::new(false)),
                generation,
                due: Instant::now(),
                active: false,
                failures: 0,
            },
        );
        Ok(initial)
    }

    pub fn health_status(&self, id: &ServerId) -> Result<HealthSnapshot, AtlasError> {
        self.health
            .lock()
            .map_err(|_| failure())?
            .tasks
            .get(id)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?
            .snapshot
            .get()
    }

    pub fn cancel_health(&self, id: &ServerId) -> Result<(), AtlasError> {
        if let Some(task) = self.health.lock().map_err(|_| failure())?.tasks.remove(id) {
            task.cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
        Ok(())
    }

    pub fn tick_health(self: &Arc<Self>) {
        let selected = {
            let Ok(mut monitor) = self.health.lock() else {
                return;
            };
            if monitor.active >= 4 {
                return;
            }
            let now = Instant::now();
            let selected = {
                let Some((id, task)) = monitor
                    .tasks
                    .iter_mut()
                    .find(|(_, task)| !task.active && task.due <= now)
                else {
                    return;
                };
                task.active = true;
                (
                    id.clone(),
                    task.config.clone(),
                    task.snapshot.clone(),
                    task.cancel.clone(),
                    task.generation,
                )
            };
            monitor.active += 1;
            selected
        };
        let operation = match self.begin() {
            Ok(value) => value,
            Err(_) => {
                if let Ok(mut monitor) = self.health.lock() {
                    monitor.active = monitor.active.saturating_sub(1);
                    if let Some(task) = monitor.tasks.get_mut(&selected.0) {
                        task.active = false;
                    }
                }
                return;
            }
        };
        let workspace = self.clone();
        let failed_id = selected.0.clone();
        if thread::Builder::new()
            .name("atlas-health-probe".into())
            .spawn(move || {
                let _operation = operation;
                let contact = workspace.live_contact_guard();
                let allowed = contact.is_ok()
                    && workspace
                        .ssh_target(&selected.0)
                        .is_ok_and(|target| target.host() == selected.1.host)
                    && !selected.3.load(std::sync::atomic::Ordering::Acquire);
                let result = if allowed {
                    probe_once(&selected.1, &selected.3)
                } else {
                    Err(AtlasError::new(ErrorCode::Cancelled))
                };
                let Ok(mut monitor) = workspace.health.lock() else {
                    return;
                };
                monitor.active = monitor.active.saturating_sub(1);
                let Some(task) = monitor
                    .tasks
                    .get_mut(&selected.0)
                    .filter(|task| task.generation == selected.4)
                else {
                    return;
                };
                task.active = false;
                match result {
                    Ok(mut snapshot) if !selected.3.load(std::sync::atomic::Ordering::Acquire) => {
                        if let Ok(previous) = selected.2.get() {
                            if snapshot.host_reachability.last_success_at.is_none() {
                                snapshot.host_reachability.last_success_at =
                                    previous.host_reachability.last_success_at;
                            }
                            if snapshot.host_reachability.last_failure_at.is_none() {
                                snapshot.host_reachability.last_failure_at =
                                    previous.host_reachability.last_failure_at;
                            }
                            if snapshot.service.last_success_at.is_none() {
                                snapshot.service.last_success_at = previous.service.last_success_at;
                            }
                            if snapshot.service.last_failure_at.is_none() {
                                snapshot.service.last_failure_at = previous.service.last_failure_at;
                            }
                        }
                        let failed = snapshot.host_reachability.state != Reachability::Reachable
                            || snapshot.service.state != Health::Healthy;
                        let delay = if failed {
                            let delay =
                                backoff_seconds(task.config.interval_seconds, task.failures);
                            task.failures = task.failures.saturating_add(1);
                            delay
                        } else {
                            task.failures = 0;
                            u64::from(task.config.interval_seconds)
                        };
                        let _ = task.snapshot.set(snapshot);
                        task.due = Instant::now() + Duration::from_secs(delay);
                    }
                    Err(error) if error.code == ErrorCode::Cancelled => {}
                    _ => {
                        task.failures = task.failures.saturating_add(1);
                        task.due = Instant::now()
                            + Duration::from_secs(backoff_seconds(
                                task.config.interval_seconds,
                                task.failures,
                            ));
                    }
                }
            })
            .is_err()
        {
            if let Ok(mut monitor) = self.health.lock() {
                monitor.active = monitor.active.saturating_sub(1);
                if let Some(task) = monitor.tasks.get_mut(&failed_id) {
                    task.active = false;
                    task.due = Instant::now() + Duration::from_secs(1);
                }
            }
        }
    }

    fn cancel_all_health(&self) {
        if let Ok(mut monitor) = self.health.lock() {
            for (_, task) in monitor.tasks.drain() {
                task.cancel
                    .store(true, std::sync::atomic::Ordering::Release);
            }
        }
    }

    pub fn import(&self) -> Result<MapState, AtlasError> {
        let path = self
            .paths
            .reports
            .as_ref()
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        let state = self.with_store(|store| {
            store.import_reports(path)?;
            store.snapshot()
        })?;
        self.apply_health(state)
    }
    fn ssh_target(&self, id: &ServerId) -> Result<ServerTarget, AtlasError> {
        if self.monitoring_state()?.records.iter().any(|r| {
            r.server_id == *id
                && (r.policy.owner_offline
                    || r.policy.lifecycle != atlas_core::monitoring::ServerLifecycle::Active)
        }) {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let state = self.with_store(|store| store.snapshot())?;
        let server = state
            .servers
            .into_iter()
            .find(|server| server.id == *id)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        ServerTarget::new(server.id, &server.host)
    }
    pub fn prepare_ssh(self: &Arc<Self>, id: &ServerId) -> Result<OperationTicket, AtlasError> {
        let _admission = self.begin()?;
        self.broker
            .prepare_connection_check(Some(&self.authority), &self.ssh_target(id)?)
    }
    pub fn verify_ssh(
        self: &Arc<Self>,
        id: &ServerId,
        operation: &OperationId,
    ) -> Result<SshVerification, AtlasError> {
        let _contact = self.live_contact_guard()?;
        self.verify_ssh_inner(id, operation)
    }
    fn verify_ssh_inner(
        self: &Arc<Self>,
        id: &ServerId,
        operation: &OperationId,
    ) -> Result<SshVerification, AtlasError> {
        let admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let alias = target.host().to_owned();
        let options = self.ssh.clone();
        let text = self.broker.execute_connection_check(
            Some(&self.authority),
            operation,
            &target,
            move |_, context| {
                // Retain the native lease for actual child/pipe cleanup, even if
                // the caller's cancellation/deadline has already returned.
                let _admission = admission;
                let result = options.verify(&alias, context)?;
                context.write(&serde_json::to_string(&result).map_err(|_| failure())?)
            },
        )?;
        if self.ssh_target(id)? != target {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        serde_json::from_str(&text).map_err(|_| failure())
    }
    pub fn cancel_ssh(&self, operation: &OperationId) -> Result<(), AtlasError> {
        self.broker.cancel(Some(&self.authority), operation)
    }
    pub fn prepare_files(self: &Arc<Self>, id: &ServerId) -> Result<OperationTicket, AtlasError> {
        let _admission = self.begin()?;
        self.files.prepare(&self.ssh_target(id)?)
    }
    pub fn open_files(
        self: &Arc<Self>,
        id: &ServerId,
        operation: &OperationId,
    ) -> Result<FileSession, AtlasError> {
        let _contact = self.live_contact_guard()?;
        let admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let inventory = self.with_store(|store| {
            let state = store.snapshot()?;
            let server = state
                .servers
                .iter()
                .find(|server| server.id == *id)
                .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
            if server.host != target.host() {
                return Err(AtlasError::new(ErrorCode::Denied));
            }
            Ok(server.inventory.clone())
        })?;
        let result =
            self.files
                .open(&target, operation, Box::new(admission), inventory.as_ref())?;
        let still_current = self.with_store(|store| {
            let state = store.snapshot()?;
            let server = state
                .servers
                .iter()
                .find(|server| server.id == *id)
                .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
            Ok(server.host == target.host() && server.inventory == inventory)
        })?;
        if !still_current || !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(&result.session_id)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(result)
    }
    pub fn refresh_inventory(
        self: &Arc<Self>,
        id: &ServerId,
    ) -> Result<InventoryRefreshResult, AtlasError> {
        let _contact = self.live_contact_guard()?;
        self.refresh_inventory_guarded(id, None, None)
    }
    fn refresh_inventory_guarded(
        self: &Arc<Self>,
        id: &ServerId,
        revision: Option<&str>,
        source_binding: Option<&str>,
    ) -> Result<InventoryRefreshResult, AtlasError> {
        self.broker.extensions().authorize(
            atlas_core::extensions::COLLECTOR_BASELINE_INVENTORY,
            atlas_core::extensions::Capability::RunApprovedCommand,
        )?;
        let _refresh_admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let ticket = self.files.prepare(&target)?;
        let session_admission = self.begin()?;
        let session = self.files.open(
            &target,
            &ticket.operation_id,
            Box::new(session_admission),
            None,
        )?;
        let collected = self.files.collect_inventory(&target, &session.session_id);
        let closed = self.files.close(&session.session_id);
        let probe = match (collected, closed) {
            (Err(failure), _) => return Err(failure),
            (Ok(_), Err(failure)) => return Err(failure),
            (Ok(probe), Ok(())) => probe,
        };
        // Keep target validation and the write under the same Store mutex.
        if let Some(expected) = source_binding {
            if probe.inventory.is_some() && probe.source_binding.as_deref() != Some(expected) {
                return Err(AtlasError::new(ErrorCode::HostTrustFailure));
            }
        }
        // Otherwise update_server could change the connection host after the
        // check but before refresh_inventory writes by stable ServerId.
        let (status, refreshed_at) = self.with_store(|store| {
            if let Some(revision) = revision {
                let allowed = store.monitoring()?.records.iter().any(|r| {
                    r.server_id == *id && r.revision == revision && r.policy.permits_contact()
                });
                if !allowed {
                    return Err(AtlasError::new(ErrorCode::Cancelled));
                }
            }
            let current = store.snapshot()?;
            let server = current
                .servers
                .iter()
                .find(|server| server.id == *id)
                .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
            if server.host != target.host() {
                return Err(AtlasError::new(ErrorCode::Denied));
            }
            let status = match probe.inventory.as_ref() {
                Some(inventory) => {
                    let source_binding = probe
                        .source_binding
                        .as_deref()
                        .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
                    if store.refresh_inventory(id, inventory, source_binding)? {
                        InventoryRefreshStatus::Updated
                    } else {
                        InventoryRefreshStatus::Unchanged
                    }
                }
                None => InventoryRefreshStatus::Partial,
            };
            let refreshed_at = store.snapshot()?.served_at;
            Ok((status, refreshed_at))
        })?;
        Ok(InventoryRefreshResult {
            contract_version: CONTRACT_VERSION,
            server_id: id.clone(),
            status,
            source_collected_at: probe.source_collected_at,
            refreshed_at,
        })
    }
    pub fn read_files(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        action: FileAction,
    ) -> Result<FileResult, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self.files.read(&target, session, action)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(result)
    }
    pub fn inspect_workload(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        workload_id: &str,
    ) -> Result<WorkloadInspection, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self.files.inspect_workload(&target, session, workload_id)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(result)
    }
    pub fn preview_workload_control(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        workload_id: &str,
        action: WorkloadAction,
    ) -> Result<WorkloadControlPreview, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self
            .files
            .preview_workload_control(&target, session, workload_id, action)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(result)
    }
    pub fn confirm_workload_control(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        operation: &OperationId,
        password: Option<ActionPassword>,
    ) -> Result<WorkloadControlReceipt, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self
            .files
            .confirm_workload_control(&target, session, operation, password)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::OutcomeUnknown));
        }
        Ok(result)
    }
    pub fn lock_files(&self, session: &SessionId) -> Result<(), AtlasError> {
        self.files.close(session)
    }
    pub fn start_terminal(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        cols: u16,
        rows: u16,
    ) -> Result<(), AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        self.files.start_terminal(&target, session, cols, rows)?;
        if self.ssh_target(id)? != target {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(())
    }
    pub fn poll_terminal(
        &self,
        id: &ServerId,
        session: &SessionId,
    ) -> Result<atlas_core::files::TerminalPoll, AtlasError> {
        self.files.poll_terminal(&self.ssh_target(id)?, session)
    }
    pub fn write_terminal(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        bytes: &[u8],
    ) -> Result<(), AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        self.files.write_terminal(&target, session, bytes)?;
        if self.ssh_target(id)? != target {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(())
    }
    pub fn resize_terminal(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        cols: u16,
        rows: u16,
    ) -> Result<(), AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        self.files.resize_terminal(&target, session, cols, rows)?;
        if self.ssh_target(id)? != target {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(())
    }
    pub fn preview_elevation(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
    ) -> Result<ElevationPreview, AtlasError> {
        let _admission = self.begin()?;
        self.files.preview_elevation(&self.ssh_target(id)?, session)
    }
    pub fn confirm_elevation(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        operation: &OperationId,
        password: ActionPassword,
    ) -> Result<ElevationReceipt, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self
            .files
            .confirm_elevation(&target, session, operation, password)?;
        if self.ssh_target(id)? != target {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(result)
    }
    pub fn preview_protected_placement(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        request: ProtectedPlacementRequest,
    ) -> Result<ElevationPreview, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self
            .files
            .preview_protected_placement(&target, session, request)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(result)
    }
    pub fn confirm_protected_placement(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        operation: &OperationId,
        password: ActionPassword,
    ) -> Result<ProtectedPlacementReceipt, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self
            .files
            .confirm_protected_placement(&target, session, operation, password)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::OutcomeUnknown));
        }
        Ok(result)
    }
    pub fn cancel_protected_placement(
        &self,
        id: &ServerId,
        session: &SessionId,
        resource: &str,
    ) -> Result<(), AtlasError> {
        self.files
            .cancel_protected_placement(&self.ssh_target(id)?, session, resource)
    }
    pub fn cancel_elevation(
        &self,
        id: &ServerId,
        session: &SessionId,
        operation: &OperationId,
    ) -> Result<(), AtlasError> {
        self.files
            .cancel_elevation(&self.ssh_target(id)?, session, operation)
    }
    pub fn release_files(&self, session: &SessionId) -> Result<(), AtlasError> {
        self.files.release_view(session)
    }
    pub fn picker_access(&self, id: &ServerId, session: &SessionId) -> Result<(), AtlasError> {
        self.files.validate_view(&self.ssh_target(id)?, session)
    }
    pub fn download_name(
        &self,
        id: &ServerId,
        session: &SessionId,
        resource: &str,
    ) -> Result<String, AtlasError> {
        self.files
            .transfer_name(&self.ssh_target(id)?, session, resource)
    }
    pub fn select_local(
        &self,
        id: &ServerId,
        session: &SessionId,
        path: PathBuf,
        direction: TransferDirection,
    ) -> Result<LocalSelection, AtlasError> {
        let target = self.ssh_target(id)?;
        self.files.validate_view(&target, session)?;
        let path_label = path
            .to_str()
            .ok_or_else(|| AtlasError::new(ErrorCode::InvalidInput))?
            .to_owned();
        let value = match direction {
            TransferDirection::Upload => LocalTransfer::upload(&path)?,
            TransferDirection::Download => LocalTransfer::download(&path)?,
        };
        let grant_id = OperationId::generate().as_str().to_owned();
        let selection = LocalSelection {
            grant_id: grant_id.clone(),
            name: value.label().into(),
            path_label,
            bytes: value.size().map(|size| size.to_string()),
            direction,
        };
        let mut grants = self.local_grants.lock().map_err(|_| failure())?;
        grants.retain(|_, grant| grant.expires > std::time::Instant::now());
        if grants.len() >= 16 {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        grants.insert(
            grant_id,
            LocalGrant {
                value,
                target,
                session: session.clone(),
                expires: std::time::Instant::now() + std::time::Duration::from_secs(10 * 60),
            },
        );
        Ok(selection)
    }
    pub fn discard_selection(&self, grant: &str) -> Result<(), AtlasError> {
        self.local_grants
            .lock()
            .map_err(|_| failure())?
            .remove(grant);
        Ok(())
    }
    pub fn enqueue_transfer(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        resource: &str,
        grant: &str,
    ) -> Result<TransferView, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        self.files.validate_view(&target, session)?;
        let mut grants = self.local_grants.lock().map_err(|_| failure())?;
        let owned = grants
            .get(grant)
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        if owned.target != target
            || owned.session != *session
            || owned.expires <= std::time::Instant::now()
        {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let owned = grants
            .remove(grant)
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        drop(grants);
        self.files
            .enqueue_transfer(&target, session, resource, owned.value)
    }
    pub fn stage_protected_placement(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        resource: &str,
        grant: &str,
    ) -> Result<ProtectedPlacementStage, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        self.files.validate_view(&target, session)?;
        let mut grants = self.local_grants.lock().map_err(|_| failure())?;
        let owned = grants
            .get(grant)
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        if owned.target != target
            || owned.session != *session
            || owned.expires <= std::time::Instant::now()
        {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let owned = grants
            .remove(grant)
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        drop(grants);
        let stage = self
            .files
            .stage_protected_upload(&target, session, resource, owned.value)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::OutcomeUnknown));
        }
        Ok(stage)
    }
    pub fn transfers(&self) -> Result<Vec<TransferView>, AtlasError> {
        self.files.transfer_views()
    }
    pub fn cancel_transfer(&self, id: &OperationId) -> Result<(), AtlasError> {
        self.files.cancel_transfer(id)
    }
    pub fn preview_change(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        change: FileChange,
    ) -> Result<FileMutationPreview, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self.files.preview_change(&target, session, change)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            self.files.close(session)?;
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(result)
    }
    pub fn commit_change(
        self: &Arc<Self>,
        id: &ServerId,
        session: &SessionId,
        operation: &OperationId,
    ) -> Result<FileMutationReceipt, AtlasError> {
        let _admission = self.begin()?;
        let target = self.ssh_target(id)?;
        let result = self.files.commit_change(&target, session, operation)?;
        if !self.ssh_target(id).is_ok_and(|current| current == target) {
            let _ = self.files.close(session);
            return Err(AtlasError::new(ErrorCode::OutcomeUnknown));
        }
        Ok(result)
    }
    pub fn discard_change(
        &self,
        id: &ServerId,
        session: &SessionId,
        operation: &OperationId,
    ) -> Result<(), AtlasError> {
        self.files
            .discard_change(&self.ssh_target(id)?, session, operation)
    }
    pub fn invalidate_server(&self, id: &ServerId) {
        self.files.close_server(id);
    }
    pub fn reap_files(&self) -> Vec<Option<SessionId>> {
        self.files.reap();
        if let Ok(mut grants) = self.local_grants.lock() {
            grants.retain(|_, grant| {
                grant.expires > std::time::Instant::now()
                    && self
                        .files
                        .validate_view(&grant.target, &grant.session)
                        .is_ok()
            });
        }
        self.files.take_closed()
    }
    pub fn is_closing(&self) -> bool {
        self.lifecycle.lock().is_ok_and(|l| l.closing)
    }
}

pub struct Operation(Arc<Workspace>);
impl Drop for Operation {
    fn drop(&mut self) {
        if let Ok(mut lifecycle) = self.0.lifecycle.lock() {
            lifecycle.active -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_policy_drains_contact_then_blocks_manual_and_background_admission() {
        use atlas_core::monitoring::MonitoringPolicy;
        let directory =
            std::env::temp_dir().join(format!("infra-policy-{}", ServerId::generate().as_str()));
        let workspace = Workspace::new(Paths {
            database: directory.join("native.sqlite3"),
            legacy: None,
            reports: None,
        });
        let id = workspace
            .with_store(|s| {
                s.add_server(AddServer {
                    name: "Offline fixture".into(),
                    host: "127.0.0.1".into(),
                })
            })
            .unwrap();
        let contact = workspace.live_contact_guard().unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = workspace.clone();
        let worker_id = id.clone();
        let thread = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = worker.set_monitoring(
                &worker_id,
                MonitoringPolicy {
                    enabled: true,
                    owner_offline: true,
                    ..Default::default()
                },
            );
            done_tx.send(result.is_ok()).unwrap();
        });
        started_rx.recv().unwrap();
        assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(contact);
        assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        thread.join().unwrap();
        assert!(workspace.prepare_ssh(&id).is_err());
        assert!(workspace.refresh_inventory(&id).is_err());
        workspace.tick_live();
        assert!(workspace.live.lock().unwrap().running.is_empty());
        assert!(
            workspace.monitoring_state().unwrap().records[0]
                .reachability
                .last_checked_at
                .is_none()
        );
        // Allow the independent config-discovery thread to finish before cleanup.
        for _ in 0..100 {
            if !workspace.live.lock().unwrap().discovery_running {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(workspace);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn persistence_and_close_admission_share_one_boundary() {
        let directory =
            std::env::temp_dir().join(format!("atlas-shell-{}", ServerId::generate().as_str()));
        let paths = Paths {
            database: directory.join("native.sqlite3"),
            legacy: None,
            reports: None,
        };
        let workspace = Workspace::new(paths.clone());
        assert!(workspace.retry().is_ok());
        let operation = workspace.begin().unwrap();
        assert!(!workspace.request_close());
        let id = workspace
            .with_store(|store| {
                store.add_server(AddServer {
                    name: "Desktop proof".into(),
                    host: "proof.invalid".into(),
                })
            })
            .unwrap();
        drop(operation);
        assert!(workspace.request_close());
        assert!(workspace.begin().is_err());
        drop(workspace);
        let reopened = Workspace::new(paths);
        assert_eq!(
            reopened
                .with_store(|store| store.snapshot())
                .unwrap()
                .servers[0]
                .id,
            id
        );
        drop(reopened);
        std::fs::remove_file(directory.join("native.sqlite3")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn failed_startup_stays_visible_without_replacing_input() {
        let paths = Paths {
            database: PathBuf::from("missing-parent-for-shell-proof/native.sqlite3"),
            legacy: Some(PathBuf::from("missing-legacy-proof.sqlite3")),
            reports: None,
        };
        let workspace = Workspace::new(paths.clone());
        assert!(!workspace.status().unwrap().storage_ready);
        assert!(workspace.retry().is_err());
        assert!(!paths.database.exists());
        assert!(workspace.import().is_err());
    }
}
