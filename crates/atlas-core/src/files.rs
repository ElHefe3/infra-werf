//! Ephemeral desktop sessions and read-only SFTP resources. One service belongs
//! to one authenticated backend principal; a session/resource DTO grants no
//! authority. Protected results never enter Store or audit records.
use crate::{
    operations::{
        Authority, ExecutionContext, Inspection, InspectionRequest, OperationBroker,
        OperationTicket, ServerTarget, SessionScope,
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
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use ts_rs::{Config, TS};

fn error(code: ErrorCode) -> AtlasError {
    AtlasError::new(code)
}
fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, AtlasError> {
    mutex.lock().map_err(|_| error(ErrorCode::IoFailure))
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
}
impl FileAction {
    fn resource(&self) -> &str {
        match self {
            Self::List { resource_id, .. }
            | Self::Stat { resource_id }
            | Self::FollowLink { resource_id } => resource_id,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileResult {
    Directory { page: FilePage },
    Metadata { entry: FileEntry },
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
    resources: HashMap<String, Resource>,
    directory: Option<Directory>,
}
struct Session {
    id: SessionId,
    target: ServerTarget,
    deadline: Instant,
    last_used: Mutex<Instant>,
    closed: AtomicBool,
    data: Mutex<Data>,
}
impl Session {
    fn valid(&self, policy: SessionPolicy) -> bool {
        !self.closed.load(Ordering::Acquire)
            && self.deadline > Instant::now()
            && self
                .last_used
                .lock()
                .is_ok_and(|last| last.elapsed() < policy.idle)
    }
}
pub struct FileSessions {
    broker: OperationBroker,
    authority: Authority,
    options: SshOptions,
    policy: SessionPolicy,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    closed: Mutex<VecDeque<Option<SessionId>>>,
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
            sessions: Mutex::new(HashMap::new()),
            closed: Mutex::new(VecDeque::new()),
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
        let id = self.broker.register_authenticated_session(
            &self.authority,
            target.clone(),
            SessionScope::new(vec![], vec![])?,
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
        };
        let resources = HashMap::from([(
            home.id.clone(),
            Resource {
                path: home_path,
                kind: FileKind::Directory,
                label: home.label.clone(),
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
        sessions.insert(
            id.as_str().into(),
            Arc::new(Session {
                id,
                target: target.clone(),
                deadline: Instant::now() + self.policy.absolute,
                last_used: Mutex::new(Instant::now()),
                closed: AtomicBool::new(false),
                data: Mutex::new(Data {
                    client,
                    resources,
                    directory: None,
                }),
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
        if !session.valid(self.policy) {
            self.close(id)?;
            return Err(error(ErrorCode::ExpiredSession));
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
                if !worker.valid(policy) {
                    return Err(error(ErrorCode::ExpiredSession));
                }
                let mut data = worker
                    .data
                    .try_lock()
                    .map_err(|_| error(ErrorCode::Conflict))?;
                let result = Self::perform(&mut data, action, context);
                match result {
                    Ok(result) => {
                        if !worker.valid(policy) {
                            return Err(error(ErrorCode::ExpiredSession));
                        }
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
                if !session.valid(self.policy) {
                    self.close(id)?;
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
                },
            );
            entries.push(entry(resource, name.attrs));
        }
        directory.cursor = if eof { None } else { Some(token()) };
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
        // Revoke before waiting for process cleanup. A concurrent read cannot
        // deliver data after lock wins the broker registry lock.
        let revoked = self.broker.close_session(&self.authority, id);
        let session = lock(&self.sessions)?.remove(id.as_str());
        if let Some(session) = session {
            session.closed.store(true, Ordering::Release);
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
    pub fn take_closed(&self) -> Vec<Option<SessionId>> {
        self.closed
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_else(|_| vec![None])
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
        FileAction::decl(&config),
        FileResult::decl(&config),
    ]
    .map(|declaration| format!("export {declaration}\n"))
    .join("")
}
