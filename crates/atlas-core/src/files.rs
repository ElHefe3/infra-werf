//! Ephemeral desktop sessions and guarded SFTP resources. One service belongs
//! to one authenticated backend principal; a session/resource DTO grants no
//! authority. Protected results never enter Store or audit records.
use crate::{
    operations::{
        Authority, ExecutionContext, Inspection, InspectionPlan, InspectionRequest,
        OperationBroker, OperationTicket, ServerTarget, SessionScope,
    },
    sftp::{self, Attrs, Client, Name},
    ssh::{SshOptions, SshProfile},
    *,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use ts_rs::{Config, TS};
mod elevation;
mod mutations;
mod transfers;
pub use elevation::{
    ActionPassword, ElevationPreview, ElevationReceipt, ProtectedPlacementReceipt,
    ProtectedPlacementRequest, ProtectedPlacementStage,
};
pub use mutations::{FileChange, FileMutationPreview, FileMutationReceipt, MutationKind};
pub use transfers::{
    LocalSelection, LocalTransfer, TransferDirection, TransferState, TransferView,
};

fn error(code: ErrorCode) -> AtlasError {
    AtlasError::new(code)
}
fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, AtlasError> {
    mutex.lock().map_err(|_| error(ErrorCode::IoFailure))
}
fn publish_if_visible<T>(
    slot: &Mutex<Option<T>>,
    visible: impl FnOnce() -> bool,
    value: T,
) -> Result<(), T> {
    let Ok(mut slot) = slot.lock() else {
        return Err(value);
    };
    if !visible() {
        return Err(value);
    }
    *slot = Some(value);
    Ok(())
}
fn token() -> String {
    SessionId::generate().as_str().into()
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct FileResource {
    pub id: String,
    pub label: String,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Other,
    Unknown,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct FileEntry {
    pub resource: FileResource,
    pub kind: FileKind,
    pub size_bytes: Option<String>,
    pub permissions: Option<String>,
    pub owner: Option<u32>,
    pub group: Option<u32>,
    pub modified_at: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct FilePage {
    pub directory: FileResource,
    pub breadcrumbs: Vec<FileResource>,
    pub entries: Vec<FileEntry>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct FileSession {
    pub session_id: SessionId,
    pub profile: SshProfile,
    pub fingerprint: String,
    pub home: FileResource,
    pub absolute_expires_at: String,
    pub idle_seconds: u32,
    pub workloads: Vec<crate::workloads::WorkloadIdentity>,
    pub workload_inventory_matches_session: bool,
}
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TerminalPoll {
    pub data: Vec<u8>,
    pub ended: bool,
    pub overflow: bool,
}
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FileAction {
    List {
        resource_id: String,
        cursor: Option<String>,
    },
    Stat {
        resource_id: String,
    },
    FollowLink {
        resource_id: String,
    },
    PinSource {
        resource_id: String,
    },
    ClearSource {
        resource_id: String,
    },
}
impl FileAction {
    fn resource(&self) -> &str {
        match self {
            Self::List { resource_id, .. }
            | Self::Stat { resource_id }
            | Self::FollowLink { resource_id }
            | Self::PinSource { resource_id }
            | Self::ClearSource { resource_id } => resource_id,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileResult {
    Directory { page: FilePage },
    Metadata { entry: FileEntry },
    Selection { resource: Option<FileResource> },
}

#[derive(Clone, Copy)]
pub struct SessionPolicy {
    idle: Duration,
    absolute: Duration,
}
impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(5 * 60),
            absolute: Duration::from_secs(30 * 60),
        }
    }
}
impl SessionPolicy {
    /// Trusted test/backend policy may only shorten product limits.
    pub fn shortened(idle: Duration, absolute: Duration) -> Result<Self, AtlasError> {
        let max = Self::default();
        if idle.is_zero() || absolute.is_zero() || idle > max.idle || absolute > max.absolute {
            return Err(error(ErrorCode::InvalidInput));
        }
        Ok(Self { idle, absolute })
    }
}
#[derive(Clone)]
struct Resource {
    path: Vec<u8>,
    kind: FileKind,
    label: String,
    mutable: bool,
}
struct Directory {
    resource: FileResource,
    path: Vec<u8>,
    handle: Vec<u8>,
    pending: VecDeque<Name>,
    cursor: Option<String>,
}
struct Data {
    client: Client,
    home_path: Vec<u8>,
    resources: HashMap<String, Resource>,
    directory: Option<Directory>,
    pinned: Option<(String, Resource)>,
    pending: Option<mutations::Pending>,
}
struct Session {
    id: SessionId,
    target: ServerTarget,
    deadline: Instant,
    last_used: Mutex<Instant>,
    closed: AtomicBool,
    visible: AtomicBool,
    terminal_starting: AtomicBool,
    terminal: Mutex<Option<crate::terminal::Process>>,
    terminal_output: Mutex<TerminalOutputState>,
    snapshot: Mutex<HashMap<String, Resource>>,
    transfers: AtomicUsize,
    stage_gate: Mutex<()>,
    staging: AtomicBool,
    close_requested: AtomicBool,
    data: Mutex<Data>,
    profile: crate::ssh::SshProfile,
    fingerprint: String,
    elevation: Mutex<Option<elevation::Pending>>,
    placement: Mutex<Option<elevation::Placement>>,
    host_key: Option<crate::ssh::conversation::HostKey>,
    workloads: HashMap<String, crate::workloads::WorkloadIdentity>,
}
const MAX_TERMINAL_OUTPUT: usize = 64 * 1024;
#[derive(Default)]
struct TerminalOutputState {
    chunks: VecDeque<Vec<u8>>,
    buffered: usize,
    ended: bool,
    overflow: bool,
}
impl TerminalOutputState {
    fn push(&mut self, bytes: Vec<u8>) {
        if self.ended || self.overflow {
            return;
        }
        if self.buffered.saturating_add(bytes.len()) > MAX_TERMINAL_OUTPUT {
            self.chunks.clear();
            self.buffered = 0;
            self.overflow = true;
            self.ended = true;
            return;
        }
        self.buffered += bytes.len();
        self.chunks.push_back(bytes);
    }
    fn poll(&mut self) -> TerminalPoll {
        if self.overflow {
            return TerminalPoll {
                data: Vec::new(),
                ended: true,
                overflow: true,
            };
        }
        let mut data = Vec::with_capacity(self.buffered);
        while let Some(chunk) = self.chunks.pop_front() {
            data.extend(chunk);
        }
        self.buffered = 0;
        TerminalPoll {
            data,
            ended: self.ended,
            overflow: false,
        }
    }
}
struct TerminalStart<'a>(&'a AtomicBool);
impl Drop for TerminalStart<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl Session {
    fn view_valid(&self, policy: SessionPolicy) -> bool {
        self.visible.load(Ordering::Acquire) && self.valid(policy)
    }
    fn valid(&self, policy: SessionPolicy) -> bool {
        !self.closed.load(Ordering::Acquire)
            && self.deadline > Instant::now()
            && self
                .last_used
                .lock()
                .is_ok_and(|last| last.elapsed() < policy.idle)
    }
    fn cleanup_valid(&self) -> bool {
        !self.closed.load(Ordering::Acquire) && self.deadline > Instant::now()
    }
}
#[derive(Clone)]
pub struct FileSessions {
    broker: OperationBroker,
    authority: Authority,
    options: SshOptions,
    policy: SessionPolicy,
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
    closed: Arc<Mutex<VecDeque<Option<SessionId>>>>,
    queue: Arc<transfers::Queue>,
}
impl FileSessions {
    pub fn new(
        broker: OperationBroker,
        authority: Authority,
        options: SshOptions,
        policy: SessionPolicy,
    ) -> Self {
        Self {
            broker,
            authority,
            options,
            policy,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            closed: Arc::new(Mutex::new(VecDeque::new())),
            queue: Arc::new(transfers::Queue::default()),
        }
    }
    pub fn prepare(&self, target: &ServerTarget) -> Result<OperationTicket, AtlasError> {
        self.reap();
        if lock(&self.sessions)?.len() >= 8 {
            return Err(error(ErrorCode::Conflict));
        }
        self.broker.prepare_sftp(Some(&self.authority), target)
    }
    pub fn open(
        &self,
        target: &ServerTarget,
        operation: &OperationId,
        lease: Box<dyn Send>,
        inventory: Option<&BaselineInventory>,
    ) -> Result<FileSession, AtlasError> {
        let slot = Arc::new(Mutex::new(None));
        let output = slot.clone();
        let options = self.options.clone();
        let alias = target.host().to_owned();
        self.broker.execute_sftp_open(
            Some(&self.authority),
            operation,
            target,
            move |_, context| {
                let mut client = Client::connect(&options, &alias, context, lease)?;
                let home = client.canonical(b"", context)?;
                context.checkpoint()?;
                *lock(&output)? = Some((client, home));
                Ok(())
            },
        )?;
        let (client, home_path) = lock(&slot)?
            .take()
            .ok_or_else(|| error(ErrorCode::IoFailure))?;
        let mut sessions = lock(&self.sessions)?;
        if sessions.len() >= 8 {
            return Err(error(ErrorCode::Conflict));
        }
        let source_binding =
            crate::inventory::authenticated_source_binding(&client.profile, &client.fingerprint);
        let workload_inventory_matches_session = inventory.is_some_and(|saved| {
            saved.source_host.as_deref() == Some(target.host())
                && saved.source_binding.as_deref() == Some(source_binding.as_str())
        });
        let workloads = crate::workloads::WorkloadIdentity::for_authenticated_session(
            target.server_id(),
            target.host(),
            &source_binding,
            inventory,
        );
        let workload_ids = workloads
            .iter()
            .map(|workload| workload.id.clone())
            .collect();
        let workload_map = workloads
            .iter()
            .cloned()
            .map(|workload| (workload.id.clone(), workload))
            .collect();
        let id = self.broker.register_authenticated_session(
            &self.authority,
            target.clone(),
            SessionScope::new(vec![], workload_ids)?,
            self.policy.absolute,
        )?;
        let home = FileResource {
            id: token(),
            label: sftp::display(&home_path),
        };
        let result = FileSession {
            session_id: id.clone(),
            profile: client.profile.clone(),
            fingerprint: client.fingerprint.clone(),
            home: home.clone(),
            absolute_expires_at: (Utc::now() + self.policy.absolute).to_rfc3339(),
            idle_seconds: self.policy.idle.as_secs().max(1) as u32,
            workloads,
            workload_inventory_matches_session,
        };
        let resources = HashMap::from([(
            home.id.clone(),
            Resource {
                path: home_path.clone(),
                kind: FileKind::Directory,
                label: home.label.clone(),
                mutable: false,
            },
        )]);
        if let Err(error) = self.broker.set_resources(
            &self.authority,
            &id,
            target,
            resources.keys().cloned().collect(),
        ) {
            let _ = self.broker.close_session(&self.authority, &id);
            return Err(error);
        }
        let host_key = client.host_key.clone();
        sessions.insert(
            id.as_str().into(),
            Arc::new(Session {
                id,
                target: target.clone(),
                deadline: Instant::now() + self.policy.absolute,
                last_used: Mutex::new(Instant::now()),
                closed: AtomicBool::new(false),
                visible: AtomicBool::new(true),
                terminal_starting: AtomicBool::new(false),
                terminal: Mutex::new(None),
                terminal_output: Mutex::new(TerminalOutputState::default()),
                snapshot: Mutex::new(resources.clone()),
                transfers: AtomicUsize::new(0),
                stage_gate: Mutex::new(()),
                staging: AtomicBool::new(false),
                close_requested: AtomicBool::new(false),
                data: Mutex::new(Data {
                    client,
                    home_path: home_path.clone(),
                    resources,
                    directory: None,
                    pinned: None,
                    pending: None,
                }),
                profile: result.profile.clone(),
                fingerprint: result.fingerprint.clone(),
                elevation: Mutex::new(None),
                placement: Mutex::new(None),
                host_key,
                workloads: workload_map,
            }),
        );
        Ok(result)
    }
    fn session(&self, target: &ServerTarget, id: &SessionId) -> Result<Arc<Session>, AtlasError> {
        let session = lock(&self.sessions)?
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| error(ErrorCode::ExpiredSession))?;
        if session.target != *target {
            return Err(error(ErrorCode::Denied));
        }
        if !session.view_valid(self.policy) {
            if !session.valid(self.policy) {
                self.close(id)?;
            }
            return Err(error(ErrorCode::ExpiredSession));
        }
        Ok(session)
    }

    /// Starts the fixed login shell only after an authenticated file session
    /// exists. A short pinned SSH probe proves public-key authentication and
    /// the same host identity before the persistent terminal connection opens.
    pub fn start_terminal(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        cols: u16,
        rows: u16,
    ) -> Result<(), AtlasError> {
        let session = self.session(target, id)?;
        if session
            .terminal_starting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(error(ErrorCode::Conflict));
        }
        let _starting = TerminalStart(&session.terminal_starting);
        let host_key = session
            .host_key
            .clone()
            .ok_or_else(|| error(ErrorCode::HostTrustFailure))?;
        let previous = {
            let mut terminal = lock(&session.terminal)?;
            if terminal
                .as_ref()
                .is_some_and(|process| !process.is_closed())
            {
                return Err(error(ErrorCode::Conflict));
            }
            terminal.take()
        };
        drop(previous);
        *lock(&session.terminal_output)? = TerminalOutputState::default();

        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::TerminalOpen {},
            },
        )?;
        let output = Arc::new(Mutex::new(None));
        let output_slot = output.clone();
        let worker = session.clone();
        let options = self.options.clone();
        let profile = session.profile.clone();
        let fingerprint = session.fingerprint.clone();
        let policy = self.policy;
        let weak_session = Arc::downgrade(&session);
        let callback = Arc::new(move |event| {
            let Some(session) = weak_session.upgrade() else {
                return;
            };
            if !session.view_valid(policy) {
                return;
            }
            match event {
                crate::terminal::Event::Data(data) => {
                    if let Ok(mut last_used) = session.last_used.lock() {
                        *last_used = Instant::now();
                    } else {
                        return;
                    }
                    if let Ok(mut output) = session.terminal_output.lock() {
                        output.push(data);
                    }
                }
                crate::terminal::Event::Ended => {
                    if let Ok(mut output) = session.terminal_output.lock() {
                        output.ended = true;
                    }
                }
            }
        });
        let executed = self.broker.execute(
            Some(&self.authority),
            &ticket.operation_id,
            target,
            move |plan, context| {
                if !matches!(plan, InspectionPlan::TerminalOpen) {
                    return Err(error(ErrorCode::Denied));
                }
                let valid = || {
                    if worker.view_valid(policy) {
                        Ok(())
                    } else {
                        Err(error(ErrorCode::ExpiredSession))
                    }
                };
                valid()?;
                let peer = options.pin_peer(&profile, &fingerprint, &host_key, context)?;
                let probe = peer.run(&["true"], None, context, &valid)?;
                if !probe.success {
                    return Err(error(ErrorCode::Unauthenticated));
                }
                let process =
                    crate::terminal::Process::start(peer, cols, rows, context, &valid, callback)?;
                *lock(&output_slot)? = Some(process);
                Ok(())
            },
        );
        if let Err(failure) = executed {
            *lock(&session.terminal_output)? = TerminalOutputState::default();
            return Err(failure);
        }
        let process = lock(&output)?
            .take()
            .ok_or_else(|| error(ErrorCode::IoFailure))?;
        if let Err(process) = publish_if_visible(
            &session.terminal,
            || session.view_valid(self.policy),
            process,
        ) {
            drop(process);
            self.close(id)?;
            return Err(error(ErrorCode::ExpiredSession));
        }
        *lock(&session.last_used)? = Instant::now();
        Ok(())
    }

    pub fn poll_terminal(
        &self,
        target: &ServerTarget,
        id: &SessionId,
    ) -> Result<TerminalPoll, AtlasError> {
        let session = self.session(target, id)?;
        if lock(&session.terminal)?.is_none() {
            return Err(error(ErrorCode::ExpiredSession));
        }
        Ok(lock(&session.terminal_output)?.poll())
    }

    pub fn write_terminal(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        bytes: &[u8],
    ) -> Result<(), AtlasError> {
        if bytes.is_empty() || bytes.len() > 16 * 1024 {
            return Err(error(ErrorCode::InvalidInput));
        }
        let session = self.session(target, id)?;
        let terminal = lock(&session.terminal)?;
        if !session.view_valid(self.policy) {
            return Err(error(ErrorCode::ExpiredSession));
        }
        terminal
            .as_ref()
            .ok_or_else(|| error(ErrorCode::ExpiredSession))?
            .write(bytes)?;
        *lock(&session.last_used)? = Instant::now();
        Ok(())
    }

    pub fn resize_terminal(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        cols: u16,
        rows: u16,
    ) -> Result<(), AtlasError> {
        let session = self.session(target, id)?;
        let terminal = lock(&session.terminal)?;
        if !session.view_valid(self.policy) {
            return Err(error(ErrorCode::ExpiredSession));
        }
        terminal
            .as_ref()
            .ok_or_else(|| error(ErrorCode::ExpiredSession))?
            .resize(cols, rows)?;
        *lock(&session.last_used)? = Instant::now();
        Ok(())
    }
    fn session_for_cleanup(
        &self,
        target: &ServerTarget,
        id: &SessionId,
    ) -> Result<Arc<Session>, AtlasError> {
        let session = lock(&self.sessions)?
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| error(ErrorCode::ExpiredSession))?;
        if session.target != *target {
            return Err(error(ErrorCode::Denied));
        }
        if !session.cleanup_valid() {
            self.close(id)?;
            return Err(error(ErrorCode::ExpiredSession));
        }
        if !lock(&session.placement)?.is_some() {
            return Err(error(ErrorCode::NotFound));
        }
        Ok(session)
    }
    pub fn read(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        action: FileAction,
    ) -> Result<FileResult, AtlasError> {
        SessionId::parse(action.resource())?;
        let session = self.session(target, id)?;
        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::SftpRead {
                    resource_id: action.resource().into(),
                },
            },
        )?;
        let worker = session.clone();
        let broker = self.broker.clone();
        let authority = self.authority.clone();
        let policy = self.policy;
        let result = self.broker.execute(
            Some(&self.authority),
            &ticket.operation_id,
            target,
            move |_, context| {
                if !worker.view_valid(policy) {
                    return Err(error(ErrorCode::ExpiredSession));
                }
                let mut data = worker
                    .data
                    .try_lock()
                    .map_err(|_| error(ErrorCode::Conflict))?;
                let result = Self::perform(&mut data, action, context);
                match result {
                    Ok(result) => {
                        if !worker.view_valid(policy) {
                            return Err(error(ErrorCode::ExpiredSession));
                        }
                        *lock(&worker.snapshot)? = data.resources.clone();
                        broker.set_resources(
                            &authority,
                            &worker.id,
                            &worker.target,
                            data.resources.keys().cloned().collect(),
                        )?;
                        let serialized = serde_json::to_string(&result)
                            .map_err(|_| error(ErrorCode::IoFailure))?;
                        context.write(&serialized)?;
                        *lock(&worker.last_used)? = Instant::now();
                        Ok(())
                    }
                    Err(failure) => Err(failure),
                }
            },
        );
        match result {
            Ok(result) => {
                if !session.view_valid(self.policy) {
                    if !session.valid(self.policy) {
                        self.close(id)?;
                    }
                    return Err(error(ErrorCode::ExpiredSession));
                }
                serde_json::from_str(&result.text).map_err(|_| error(ErrorCode::IoFailure))
            }
            Err(failure) => {
                if !matches!(
                    failure.code,
                    ErrorCode::Denied
                        | ErrorCode::NotFound
                        | ErrorCode::InvalidInput
                        | ErrorCode::Conflict
                ) {
                    let _ = self.close(id);
                }
                Err(failure)
            }
        }
    }

    /// Collect the fixed approved baseline over a host-pinned SSH exec channel
    /// while an authenticated SFTP session supplies the protected lifetime.
    /// Raw remote output is parsed inside the trusted adapter and never leaves
    /// as a frontend or persistence DTO.
    pub fn collect_inventory(
        &self,
        target: &ServerTarget,
        id: &SessionId,
    ) -> Result<crate::inventory::InventoryProbe, AtlasError> {
        let session = self.session(target, id)?;
        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::SystemSummary {},
            },
        )?;
        let worker = session.clone();
        let options = self.options.clone();
        let profile = session.profile.clone();
        let fingerprint = session.fingerprint.clone();
        let source_binding = crate::inventory::authenticated_source_binding(&profile, &fingerprint);
        let host_key = session
            .host_key
            .clone()
            .ok_or_else(|| error(ErrorCode::HostTrustFailure))?;
        let policy = self.policy;
        let result = self.broker.execute(
            Some(&self.authority),
            &ticket.operation_id,
            target,
            move |plan, context| {
                if !matches!(plan, InspectionPlan::SystemSummary) {
                    return Err(error(ErrorCode::Denied));
                }
                let valid = || {
                    if worker.view_valid(policy) {
                        Ok(())
                    } else {
                        Err(error(ErrorCode::ExpiredSession))
                    }
                };
                valid()?;
                let peer = options.pin_peer(&profile, &fingerprint, &host_key, context)?;
                let reply = peer.run(
                    &["/bin/bash", "-c", crate::inventory::REMOTE_BASELINE_COMMAND],
                    None,
                    context,
                    &valid,
                )?;
                let probe = if reply.success {
                    if let Some(delay) =
                        crate::inventory::remote_clock_delay(&reply.stdout, Utc::now())
                    {
                        let until = Instant::now() + delay;
                        while Instant::now() < until {
                            context.checkpoint()?;
                            valid()?;
                            std::thread::sleep(
                                until
                                    .saturating_duration_since(Instant::now())
                                    .min(Duration::from_millis(25)),
                            );
                        }
                    }
                    context.checkpoint()?;
                    valid()?;
                    crate::inventory::parse_remote_baseline(&reply.stdout, Utc::now())
                } else {
                    crate::inventory::InventoryProbe {
                        inventory: None,
                        source_collected_at: None,
                        source_binding: None,
                    }
                };
                let mut probe = probe;
                if probe.inventory.is_some() {
                    probe.source_binding = Some(source_binding.clone());
                }
                let encoded =
                    serde_json::to_string(&probe).map_err(|_| error(ErrorCode::IoFailure))?;
                context.write(&encoded)?;
                *lock(&worker.last_used)? = Instant::now();
                Ok(())
            },
        );
        match result {
            Ok(value) => serde_json::from_str(&value.text).map_err(|_| error(ErrorCode::IoFailure)),
            Err(failure) if failure.code == ErrorCode::OutputLimit => {
                Ok(crate::inventory::InventoryProbe {
                    inventory: None,
                    source_collected_at: None,
                    source_binding: None,
                })
            }
            Err(failure) => Err(failure),
        }
    }

    /// Read-only application discovery (F17/T035): the fixed
    /// `REMOTE_APPS_COMMAND` over this authenticated, host-pinned session.
    /// Only parsed port/bind/status/title/process fields are returned.
    pub fn discover_applications(
        &self,
        target: &ServerTarget,
        id: &SessionId,
    ) -> Result<(bool, Vec<crate::tunnel::DiscoveredApp>), AtlasError> {
        let session = self.session(target, id)?;
        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::ApplicationDiscovery {},
            },
        )?;
        let worker = session.clone();
        let options = self.options.clone();
        let profile = session.profile.clone();
        let fingerprint = session.fingerprint.clone();
        let host_key = session
            .host_key
            .clone()
            .ok_or_else(|| error(ErrorCode::HostTrustFailure))?;
        let policy = self.policy;
        let result = self.broker.execute(
            Some(&self.authority),
            &ticket.operation_id,
            target,
            move |plan, context| {
                if !matches!(plan, InspectionPlan::ApplicationDiscovery) {
                    return Err(error(ErrorCode::Denied));
                }
                let valid = || {
                    if worker.view_valid(policy) {
                        Ok(())
                    } else {
                        Err(error(ErrorCode::ExpiredSession))
                    }
                };
                valid()?;
                let peer = options.pin_peer(&profile, &fingerprint, &host_key, context)?;
                let reply = peer.run(
                    &["/bin/bash", "-c", crate::tunnel::REMOTE_APPS_COMMAND],
                    None,
                    context,
                    &valid,
                )?;
                context.checkpoint()?;
                valid()?;
                let parsed = if reply.success {
                    crate::tunnel::parse_discovery(&reply.stdout)
                } else {
                    None
                };
                let (complete, apps) = parsed.ok_or_else(|| error(ErrorCode::RemoteRejected))?;
                let encoded = serde_json::to_string(&(complete, apps))
                    .map_err(|_| error(ErrorCode::IoFailure))?;
                context.write(&encoded)?;
                *lock(&worker.last_used)? = Instant::now();
                Ok(())
            },
        )?;
        serde_json::from_str(&result.text).map_err(|_| error(ErrorCode::IoFailure))
    }

    /// Inspect one saved workload through its authenticated, host-pinned SSH
    /// session. The typed identity is reconstructed from trusted inventory at
    /// session open; no caller field is ever used as a remote command.
    pub fn inspect_workload(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        workload_id: &str,
    ) -> Result<crate::workloads::WorkloadInspection, AtlasError> {
        let session = self.session(target, id)?;
        let identity = session
            .workloads
            .get(workload_id)
            .cloned()
            .ok_or_else(|| error(ErrorCode::Denied))?;
        if !identity.supported {
            return Ok(crate::workloads::WorkloadInspection::unsupported(
                workload_id,
            ));
        }
        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::ServiceStatus {
                    workload_id: workload_id.to_owned(),
                },
            },
        )?;
        let worker = session.clone();
        let options = self.options.clone();
        let profile = session.profile.clone();
        let fingerprint = session.fingerprint.clone();
        let host_key = session
            .host_key
            .clone()
            .ok_or_else(|| error(ErrorCode::HostTrustFailure))?;
        let policy = self.policy;
        let broker_result = self.broker.execute(
            Some(&self.authority),
            &ticket.operation_id,
            target,
            move |plan, context| {
                if !matches!(plan, InspectionPlan::ServiceStatus { workload_id: id } if id == &identity.id)
                {
                    return Err(error(ErrorCode::Denied));
                }
                let valid = || {
                    if worker.view_valid(policy) {
                        Ok(())
                    } else {
                        Err(error(ErrorCode::ExpiredSession))
                    }
                };
                valid()?;
                let peer = options.pin_peer(&profile, &fingerprint, &host_key, context)?;
                let kind = match identity.kind {
                    crate::workloads::WorkloadKind::Service => "service",
                    crate::workloads::WorkloadKind::Container => "container",
                };
                let reply = peer.run(
                    &[
                        "/bin/bash",
                        "-c",
                        crate::workloads::REMOTE_WORKLOAD_INSPECTION,
                        "atlas-workload",
                        kind,
                        &identity.scope,
                        &identity.runtime,
                        &identity.name,
                    ],
                    None,
                    context,
                    &valid,
                )?;
                if !reply.success {
                    return Err(error(ErrorCode::IoFailure));
                }
                let inspection = crate::workloads::parse_workload_inspection(
                    &reply.stdout,
                    &identity.id,
                )?;
                let encoded =
                    serde_json::to_string(&inspection).map_err(|_| error(ErrorCode::IoFailure))?;
                context.write(&encoded)?;
                *lock(&worker.last_used)? = Instant::now();
                Ok(())
            },
        );
        match broker_result {
            Ok(value) => {
                if !session.view_valid(self.policy) {
                    if !session.valid(self.policy) {
                        self.close(id)?;
                    }
                    return Err(error(ErrorCode::ExpiredSession));
                }
                serde_json::from_str(&value.text).map_err(|_| error(ErrorCode::IoFailure))
            }
            Err(failure) => {
                if !matches!(
                    failure.code,
                    ErrorCode::Denied
                        | ErrorCode::NotFound
                        | ErrorCode::InvalidInput
                        | ErrorCode::Conflict
                ) {
                    let _ = self.close(id);
                }
                Err(failure)
            }
        }
    }

    fn perform(
        data: &mut Data,
        action: FileAction,
        context: &mut ExecutionContext,
    ) -> Result<FileResult, AtlasError> {
        let resource = data
            .resources
            .get(action.resource())
            .cloned()
            .ok_or_else(|| error(ErrorCode::Denied))?;
        match action {
            FileAction::PinSource { .. } => {
                if !resource.mutable {
                    return Err(error(ErrorCode::Denied));
                }
                if let Some((old, _)) = data.pinned.take() {
                    data.resources.remove(&old);
                }
                data.pending = None;
                let id = token();
                data.resources.insert(id.clone(), resource.clone());
                let label = sftp::display(&resource.path);
                data.pinned = Some((id.clone(), resource));
                Ok(FileResult::Selection {
                    resource: Some(FileResource { id, label }),
                })
            }
            FileAction::ClearSource { resource_id } => {
                if data.pinned.as_ref().map(|(id, _)| id) != Some(&resource_id) {
                    return Err(error(ErrorCode::Denied));
                }
                data.pinned = None;
                data.pending = None;
                data.resources.remove(&resource_id);
                Ok(FileResult::Selection { resource: None })
            }
            FileAction::Stat { resource_id } => {
                let attrs = data.client.stat(&resource.path, context)?;
                Ok(FileResult::Metadata {
                    entry: entry(
                        FileResource {
                            id: resource_id,
                            label: resource.label,
                        },
                        attrs,
                    ),
                })
            }
            FileAction::FollowLink { .. } => {
                if resource.kind != FileKind::Symlink {
                    return Err(error(ErrorCode::InvalidInput));
                }
                let path = data.client.canonical(&resource.path, context)?;
                let attrs = data.client.stat(&path, context)?;
                if kind(&attrs) != FileKind::Directory {
                    return Err(error(ErrorCode::Denied));
                }
                Self::start_directory(data, path, context)
            }
            FileAction::List {
                resource_id,
                cursor,
            } => {
                if resource.kind != FileKind::Directory {
                    return Err(error(ErrorCode::Denied));
                }
                if let Some(cursor) = cursor {
                    SessionId::parse(&cursor)?;
                    let directory = data
                        .directory
                        .as_mut()
                        .ok_or_else(|| error(ErrorCode::Denied))?;
                    if directory.resource.id != resource_id
                        || directory.cursor.as_deref() != Some(&cursor)
                    {
                        return Err(error(ErrorCode::Denied));
                    }
                    directory.cursor = None;
                    Self::page(data, context)
                } else {
                    // Explicit navigation resolves the selected directory.
                    // Scope is the SSH account filesystem, not a claimed jail.
                    let path = data.client.canonical(&resource.path, context)?;
                    Self::start_directory(data, path, context)
                }
            }
        }
    }
    fn start_directory(
        data: &mut Data,
        path: Vec<u8>,
        context: &mut ExecutionContext,
    ) -> Result<FileResult, AtlasError> {
        // Keep the previous page usable if opening the new directory is denied.
        let handle = data.client.open_dir(&path, context)?;
        data.pending = None;
        let previous = data.directory.take();
        let resource = FileResource {
            id: token(),
            label: sftp::display(&path),
        };
        data.directory = Some(Directory {
            resource,
            path,
            handle,
            pending: VecDeque::new(),
            cursor: None,
        });
        match Self::page(data, context) {
            Ok(page) => {
                if let Some(previous) = previous {
                    data.client.close_dir(&previous.handle, context)?;
                }
                Ok(page)
            }
            Err(failure) => {
                if matches!(failure.code, ErrorCode::Denied | ErrorCode::NotFound) {
                    if let Some(candidate) = data.directory.take() {
                        data.client.close_dir(&candidate.handle, context)?;
                    }
                    data.directory = previous;
                }
                Err(failure)
            }
        }
    }
    fn page(data: &mut Data, context: &mut ExecutionContext) -> Result<FileResult, AtlasError> {
        let directory = data
            .directory
            .as_mut()
            .ok_or_else(|| error(ErrorCode::Denied))?;
        let mut resources = HashMap::new();
        let mut breadcrumbs = Vec::new();
        let mut path = Vec::new();
        // At most 64 breadcrumb resources; a deeper legitimate directory can
        // still be displayed but receives a fixed output-limit failure.
        for part in std::iter::once(b"".as_slice()).chain(
            directory.path[1..]
                .split(|byte| *byte == b'/')
                .filter(|part| !part.is_empty()),
        ) {
            if breadcrumbs.len() >= 64 {
                return Err(error(ErrorCode::OutputLimit));
            }
            if path.last() != Some(&b'/') {
                path.push(b'/');
            }
            path.extend(part);
            let resource = FileResource {
                id: token(),
                label: if part.is_empty() {
                    "/".into()
                } else {
                    sftp::display(part)
                },
            };
            resources.insert(
                resource.id.clone(),
                Resource {
                    path: path.clone(),
                    kind: FileKind::Directory,
                    label: resource.label.clone(),
                    mutable: false,
                },
            );
            breadcrumbs.push(resource);
        }
        resources.insert(
            directory.resource.id.clone(),
            Resource {
                path: directory.path.clone(),
                kind: FileKind::Directory,
                label: directory.resource.label.clone(),
                mutable: false,
            },
        );
        let mut entries = Vec::new();
        let mut bytes = 0;
        let mut eof = false;
        while entries.len() < 100 {
            context.checkpoint()?;
            if directory.pending.is_empty() {
                match data.client.read_dir(&directory.handle, context)? {
                    Some(names) if names.is_empty() => return Err(error(ErrorCode::IoFailure)),
                    Some(names) => directory.pending.extend(names),
                    None => {
                        eof = true;
                        break;
                    }
                }
            }
            let Some(name) = directory.pending.pop_front() else {
                continue;
            };
            if name.name == b"." || name.name == b".." {
                continue;
            }
            if name.name.is_empty() || name.name.contains(&0) || name.name.contains(&b'/') {
                return Err(error(ErrorCode::IoFailure));
            }
            // JSON escaping can multiply input bytes by six. Reserve enough
            // space for path/breadcrumbs within the broker's 64KiB response.
            let cost = sftp::display(&name.name).len() * 6 + 512;
            if bytes + cost > 28 * 1024 && !entries.is_empty() {
                directory.pending.push_front(name);
                break;
            }
            bytes += cost;
            let mut path = directory.path.clone();
            if path.last() != Some(&b'/') {
                path.push(b'/');
            }
            path.extend(&name.name);
            if path.len() > 4096 {
                return Err(error(ErrorCode::OutputLimit));
            }
            let resource = FileResource {
                id: token(),
                label: sftp::display(&name.name),
            };
            resources.insert(
                resource.id.clone(),
                Resource {
                    path,
                    kind: kind(&name.attrs),
                    label: resource.label.clone(),
                    mutable: true,
                },
            );
            entries.push(entry(resource, name.attrs));
        }
        directory.cursor = if eof { None } else { Some(token()) };
        if let Some((id, resource)) = &data.pinned {
            resources.insert(id.clone(), resource.clone());
        }
        data.resources = resources;
        Ok(FileResult::Directory {
            page: FilePage {
                directory: directory.resource.clone(),
                breadcrumbs,
                entries,
                next_cursor: directory.cursor.clone(),
            },
        })
    }
    pub fn close(&self, id: &SessionId) -> Result<(), AtlasError> {
        let active = lock(&self.sessions)?.get(id.as_str()).cloned();
        let Some(active) = active else {
            return self.broker.close_session(&self.authority, id);
        };
        // Serialize stage admission with revocation. Whichever acquires this
        // gate first either registers staging before Lock can inspect it, or
        // sees the closed/hidden session before any remote stage begins.
        let _stage_gate = lock(&active.stage_gate)?;
        let protected_stage_pending = active.staging.load(Ordering::Acquire)
            || (active.cleanup_valid() && lock(&active.placement)?.is_some());
        if protected_stage_pending {
            active.close_requested.store(true, Ordering::Release);
            active.visible.store(false, Ordering::Release);
            lock(&active.snapshot)?.clear();
            Self::stop_terminal(&active)?;
            return Ok(());
        }
        // Revoke before waiting for process cleanup. A concurrent read cannot
        // deliver data after lock wins the broker registry lock.
        active.visible.store(false, Ordering::Release);
        let revoked = self.broker.close_session(&self.authority, id);
        let session = lock(&self.sessions)?.remove(id.as_str());
        if let Some(session) = session {
            session.closed.store(true, Ordering::Release);
            Self::stop_terminal(&session)?;
            let mut events = lock(&self.closed)?;
            if events.len() >= 128 {
                // Bounded overflow requests clearing all protected views.
                events.clear();
                events.push_back(None);
            } else {
                events.push_back(Some(id.clone()));
            }
            drop(events);
            drop(session);
        }
        revoked
    }
    fn stop_terminal(session: &Session) -> Result<(), AtlasError> {
        *lock(&session.terminal_output)? = TerminalOutputState::default();
        let terminal = lock(&session.terminal)?.take();
        drop(terminal);
        Ok(())
    }
    pub fn take_closed(&self) -> Vec<Option<SessionId>> {
        self.closed
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_else(|_| vec![None])
    }
    pub fn release_view(&self, id: &SessionId) -> Result<(), AtlasError> {
        let session = lock(&self.sessions)?.get(id.as_str()).cloned();
        if let Some(session) = session {
            session.visible.store(false, Ordering::Release);
            lock(&session.snapshot)?.clear();
            session.close_requested.store(true, Ordering::Release);
            Self::stop_terminal(&session)?;
            if session.transfers.load(Ordering::Acquire) == 0 {
                return self.close(id);
            }
        }
        Ok(())
    }
    pub fn close_server(&self, id: &ServerId) {
        let ids = self
            .sessions
            .lock()
            .map(|sessions| {
                sessions
                    .values()
                    .filter(|s| s.target.server_id() == id)
                    .map(|s| s.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for id in ids {
            let _ = self.close(&id);
        }
    }
    /// Reap idle/absolute/disconnected sessions without refreshing activity.
    pub fn reap(&self) -> Vec<SessionId> {
        self.queue.scrub(self.policy);
        let ids = self
            .sessions
            .lock()
            .map(|sessions| {
                sessions
                    .values()
                    .filter(|s| {
                        !s.valid(self.policy)
                            || self
                                .broker
                                .validate_session(&self.authority, &s.id, &s.target)
                                .is_err()
                            || s.data.try_lock().is_ok_and(|mut data| !data.client.alive())
                    })
                    .map(|s| s.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for id in &ids {
            let cleanup = lock(&self.sessions).ok().and_then(|sessions| {
                sessions.get(id.as_str()).and_then(|session| {
                    if session.cleanup_valid() && !session.staging.load(Ordering::Acquire) {
                        lock(&session.placement)
                            .ok()
                            .and_then(|placement| placement.clone())
                            .map(|placement| (session.target.clone(), placement.resource_id))
                    } else {
                        None
                    }
                })
            });
            if let Some((target, resource_id)) = cleanup {
                // Idle expiry revokes browsing but retains the SSH channel only
                // for this backend-recorded private-stage cleanup. Absolute
                // expiry below still closes the channel even if cleanup fails.
                let _ = self.cancel_protected_placement(&target, id, &resource_id);
            }
            let _ = self.close(id);
        }
        ids
    }
}
fn kind(attrs: &Attrs) -> FileKind {
    match attrs.permissions.map(|mode| mode & 0o170000) {
        Some(0o040000) => FileKind::Directory,
        Some(0o100000) => FileKind::File,
        Some(0o120000) => FileKind::Symlink,
        Some(_) => FileKind::Other,
        None => FileKind::Unknown,
    }
}
fn entry(resource: FileResource, attrs: Attrs) -> FileEntry {
    FileEntry {
        resource,
        kind: kind(&attrs),
        size_bytes: attrs.size.map(|size| size.to_string()),
        permissions: attrs
            .permissions
            .map(|mode| format!("{:04o}", mode & 0o7777)),
        owner: attrs.uid,
        group: attrs.gid,
        modified_at: attrs
            .modified
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds as i64, 0))
            .map(|date| date.to_rfc3339()),
    }
}
pub fn typescript_contracts() -> String {
    let config = Config::default();
    [
        FileResource::decl(&config),
        FileKind::decl(&config),
        FileEntry::decl(&config),
        FilePage::decl(&config),
        FileSession::decl(&config),
        TerminalPoll::decl(&config),
        FileAction::decl(&config),
        FileResult::decl(&config),
    ]
    .map(|declaration| format!("export {declaration}\n"))
    .join("")
        + &mutations::typescript_contracts()
        + &transfers::typescript_contracts()
        + &elevation::typescript_contracts()
}

#[cfg(test)]
mod terminal_output_tests {
    use super::*;

    #[test]
    fn terminal_output_is_drained_in_memory_and_capped_at_64_kib() {
        let mut output = TerminalOutputState::default();
        output.push(vec![b'a'; 32 * 1024]);
        output.push(vec![b'b'; 32 * 1024]);
        let drained = output.poll();
        assert_eq!(drained.data.len(), MAX_TERMINAL_OUTPUT);
        assert!(!drained.ended);
        assert!(!drained.overflow);
        assert_eq!(output.buffered, 0);

        output.push(vec![b'x'; MAX_TERMINAL_OUTPUT]);
        output.push(vec![b'y']);
        let overflow = output.poll();
        assert!(overflow.ended);
        assert!(overflow.overflow);
        assert!(overflow.data.is_empty());
        assert_eq!(output.buffered, 0);
    }
}

#[cfg(test)]
mod terminal_publication_tests {
    use super::*;

    #[test]
    fn lock_winning_the_terminal_slot_prevents_late_process_publication() {
        let slot = Arc::new(Mutex::new(None));
        let visible = Arc::new(AtomicBool::new(true));
        let held = slot.lock().unwrap();
        let worker_slot = slot.clone();
        let worker_visible = visible.clone();
        let worker = std::thread::spawn(move || {
            publish_if_visible(
                &worker_slot,
                || worker_visible.load(Ordering::Acquire),
                "shell",
            )
        });

        visible.store(false, Ordering::Release);
        drop(held);

        assert_eq!(worker.join().unwrap(), Err("shell"));
        assert!(slot.lock().unwrap().is_none());
    }
}
