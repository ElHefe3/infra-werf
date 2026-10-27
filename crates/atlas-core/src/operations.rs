//! Backend-only authority and bounded, read-only managed operation dispatch.
//! No IPC caller can mint authority/session grants; F05/F06 must do so only
//! after their respective authentication checks. No network executor here.
use crate::*;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use ts_rs::{Config, TS};

const MAX_SESSIONS: usize = 128;
const MAX_JOBS: usize = 1024;
const MAX_WORKERS: usize = 4;
const MAX_OUTPUT: usize = 64 * 1024;
const MAX_DURATION: Duration = Duration::from_secs(30);

fn error(code: ErrorCode) -> AtlasError {
    AtlasError::new(code)
}
fn lock<T>(value: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, AtlasError> {
    value.lock().map_err(|_| error(ErrorCode::IoFailure))
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Inspection {
    SystemSummary {},
    FileMetadata { path: String },
    ServiceStatus { workload_id: String },
    WorkloadControl { workload_id: String },
    SftpRead { resource_id: String },
    SftpMutation { resource_id: String },
    SftpTransfer { resource_id: String },
    ElevationCheck {},
    ProtectedPlacement {},
    TerminalOpen {},
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InspectionRequest {
    pub server_id: ServerId,
    pub session_id: SessionId,
    pub inspection: Inspection,
}
impl InspectionRequest {
    /// Bounded/redacted decoder for future transport adapters. Do not forward
    /// serde's raw input diagnostics through IPC or coordinator responses.
    pub fn decode(input: &[u8]) -> Result<Self, AtlasError> {
        if input.len() > 32 * 1024 {
            return Err(error(ErrorCode::InvalidInput));
        }
        serde_json::from_slice(input).map_err(|_| error(ErrorCode::InvalidInput))
    }
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationTicket {
    pub operation_id: OperationId,
    pub expires_at: String,
}

/// A backend snapshot of the selected server, never supplied by the frontend.
#[derive(Clone, PartialEq, Eq)]
pub struct ServerTarget {
    server_id: ServerId,
    host: String,
}
impl ServerTarget {
    pub fn new(server_id: ServerId, host: &str) -> Result<Self, AtlasError> {
        let valid = hostname(host)?;
        if valid != host {
            return Err(error(ErrorCode::InvalidInput));
        }
        Ok(Self {
            server_id,
            host: valid,
        })
    }
    pub fn server_id(&self) -> &ServerId {
        &self.server_id
    }
    pub fn host(&self) -> &str {
        &self.host
    }
}

/// Opaque in-memory proof owned by a trusted backend adapter. No Deserialize,
/// Serialize or secret-bearing Debug implementation; a DTO grants no access.
#[derive(Clone)]
pub struct Authority {
    broker: String,
    id: String,
}

#[derive(Clone)]
pub struct ScopedPath {
    root: String,
    path: String,
}
fn absolute_path(path: &str) -> Result<String, AtlasError> {
    if path.len() > 4096
        || !path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .skip(1)
            .any(|part| part == "." || part == ".." || part.is_empty())
            && path != "/"
    {
        return Err(error(ErrorCode::InvalidInput));
    }
    Ok(path.to_owned())
}
fn within(root: &str, path: &str) -> bool {
    root == "/"
        || root == path
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}
impl ScopedPath {
    pub fn path(&self) -> &str {
        &self.path
    }
    /// SFTP executors MUST obtain the server's canonical path and check it
    /// before stat/read. Lexical validation alone cannot confine symlinks.
    pub fn check_resolved(&self, canonical: &str) -> Result<(), AtlasError> {
        let canonical = absolute_path(canonical)?;
        if within(&self.root, &canonical) {
            Ok(())
        } else {
            Err(error(ErrorCode::Denied))
        }
    }
}

#[derive(Clone)]
pub enum InspectionPlan {
    ConnectionCheck,
    SftpOpen,
    SftpRead { resource_id: String },
    SftpMutation { resource_id: String },
    SftpTransfer { resource_id: String },
    ElevationCheck,
    ProtectedPlacement,
    TerminalOpen,
    SystemSummary,
    FileMetadata(ScopedPath),
    ServiceStatus { workload_id: String },
    WorkloadControl { workload_id: String },
}

/// Scopes originate in authenticated discovery, not in an operation request.
pub struct SessionScope {
    roots: Vec<String>,
    workloads: HashSet<String>,
    resources: HashSet<String>,
}
impl SessionScope {
    pub fn new(roots: Vec<String>, workloads: Vec<String>) -> Result<Self, AtlasError> {
        if roots.len() > 32 || workloads.len() > 2048 {
            return Err(error(ErrorCode::InvalidInput));
        }
        let roots = roots
            .into_iter()
            .map(|root| absolute_path(&root))
            .collect::<Result<_, _>>()?;
        let workloads = workloads
            .into_iter()
            .map(|id| {
                if text(&id, 256, false)? != id || id.chars().any(char::is_control) {
                    return Err(error(ErrorCode::InvalidInput));
                }
                Ok(id)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            roots,
            workloads,
            resources: HashSet::new(),
        })
    }
    fn plan(&self, inspection: Inspection) -> Result<InspectionPlan, AtlasError> {
        Ok(match inspection {
            Inspection::ElevationCheck {} => InspectionPlan::ElevationCheck,
            Inspection::ProtectedPlacement {} => InspectionPlan::ProtectedPlacement,
            Inspection::TerminalOpen {} => InspectionPlan::TerminalOpen,
            Inspection::SftpTransfer { resource_id } => {
                if !self.resources.contains(&resource_id) {
                    return Err(error(ErrorCode::Denied));
                }
                InspectionPlan::SftpTransfer { resource_id }
            }
            Inspection::SftpMutation { resource_id } => {
                if !self.resources.contains(&resource_id) {
                    return Err(error(ErrorCode::Denied));
                }
                InspectionPlan::SftpMutation { resource_id }
            }
            Inspection::SftpRead { resource_id } => {
                if !self.resources.contains(&resource_id) {
                    return Err(error(ErrorCode::Denied));
                }
                InspectionPlan::SftpRead { resource_id }
            }
            Inspection::SystemSummary {} => InspectionPlan::SystemSummary,
            Inspection::FileMetadata { path } => {
                let path = absolute_path(&path)?;
                let root = self
                    .roots
                    .iter()
                    .filter(|root| within(root, &path))
                    .max_by_key(|root| root.len())
                    .ok_or_else(|| error(ErrorCode::Denied))?;
                InspectionPlan::FileMetadata(ScopedPath {
                    root: root.clone(),
                    path,
                })
            }
            Inspection::ServiceStatus { workload_id } => {
                if !self.workloads.contains(&workload_id) {
                    return Err(error(ErrorCode::Denied));
                }
                InspectionPlan::ServiceStatus { workload_id }
            }
            Inspection::WorkloadControl { workload_id } => {
                if !self.workloads.contains(&workload_id) {
                    return Err(error(ErrorCode::Denied));
                }
                InspectionPlan::WorkloadControl { workload_id }
            }
        })
    }
}

struct Session {
    actor: String,
    target: ServerTarget,
    scope: SessionScope,
    expires: Instant,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Prepared,
    Running,
    Finished,
}
struct Job {
    actor: String,
    session_id: Option<SessionId>,
    target: ServerTarget,
    plan: InspectionPlan,
    expires: Instant,
    phase: Mutex<Phase>,
    cancelled: AtomicBool,
}

/// Safe audit metadata only; never executor output, arguments or credentials.
#[derive(Clone)]
pub struct OperationAudit {
    pub operation_id: OperationId,
    pub server_id: ServerId,
    pub failure: Option<ErrorCode>,
}
#[derive(Default)]
struct Registry {
    actors: HashSet<String>,
    sessions: HashMap<String, Session>,
    jobs: HashMap<String, Arc<Job>>,
    audit: VecDeque<OperationAudit>,
}
struct Shared {
    registry: Mutex<Registry>,
    workers: AtomicUsize,
}
#[derive(Clone)]
pub struct OperationBroker {
    id: String,
    shared: Arc<Shared>,
    duration: Duration,
    output_limit: usize,
}

impl Default for OperationBroker {
    fn default() -> Self {
        Self::new(MAX_DURATION, MAX_OUTPUT).expect("valid fixed operation budgets")
    }
}
impl OperationBroker {
    /// Budgets may only be lowered by a backend/test fixture, never by a DTO.
    pub fn new(duration: Duration, output_limit: usize) -> Result<Self, AtlasError> {
        if duration.is_zero()
            || duration > MAX_DURATION
            || output_limit == 0
            || output_limit > MAX_OUTPUT
        {
            return Err(error(ErrorCode::InvalidInput));
        }
        Ok(Self {
            id: SessionId::generate().as_str().into(),
            shared: Arc::new(Shared {
                registry: Mutex::new(Registry::default()),
                workers: AtomicUsize::new(0),
            }),
            duration,
            output_limit,
        })
    }
    /// Native code calls this for its trusted local window. A future remote
    /// adapter may call it ONLY after validated pairing/session authentication.
    pub fn issue_backend_authority(&self) -> Result<Authority, AtlasError> {
        let id = SessionId::generate().as_str().to_owned();
        let mut registry = lock(&self.shared.registry)?;
        if registry.actors.len() >= MAX_SESSIONS {
            return Err(error(ErrorCode::Conflict));
        }
        registry.actors.insert(id.clone());
        Ok(Authority {
            broker: self.id.clone(),
            id,
        })
    }
    fn actor<'a>(
        &self,
        registry: &Registry,
        authority: Option<&'a Authority>,
    ) -> Result<&'a Authority, AtlasError> {
        let authority = authority.ok_or_else(|| error(ErrorCode::Unauthenticated))?;
        if authority.broker != self.id || !registry.actors.contains(&authority.id) {
            return Err(error(ErrorCode::Denied));
        }
        Ok(authority)
    }
    /// Called only AFTER actual SSH authentication, never to implement it.
    /// T013 will supply idle/absolute lifetime policy and protected UI clearing.
    pub fn register_authenticated_session(
        &self,
        authority: &Authority,
        target: ServerTarget,
        scope: SessionScope,
        lifetime: Duration,
    ) -> Result<SessionId, AtlasError> {
        if lifetime.is_zero() || lifetime > Duration::from_secs(30 * 60) {
            return Err(error(ErrorCode::InvalidInput));
        }
        let mut registry = lock(&self.shared.registry)?;
        self.actor(&registry, Some(authority))?;
        registry
            .sessions
            .retain(|_, session| session.expires > Instant::now());
        if registry.sessions.len() >= MAX_SESSIONS {
            return Err(error(ErrorCode::Conflict));
        }
        let id = SessionId::generate();
        registry.sessions.insert(
            id.as_str().into(),
            Session {
                actor: authority.id.clone(),
                target,
                scope,
                expires: Instant::now() + lifetime,
            },
        );
        Ok(id)
    }
    pub fn revoke(&self, authority: &Authority) -> Result<(), AtlasError> {
        let mut registry = lock(&self.shared.registry)?;
        self.actor(&registry, Some(authority))?;
        registry.actors.remove(&authority.id);
        registry
            .sessions
            .retain(|_, session| session.actor != authority.id);
        for job in registry
            .jobs
            .values()
            .filter(|job| job.actor == authority.id)
        {
            job.cancelled.store(true, Ordering::Release);
        }
        Ok(())
    }
    /// Backend-owned opaque resources, never paths supplied by an IPC caller.
    pub fn set_resources(
        &self,
        authority: &Authority,
        id: &SessionId,
        target: &ServerTarget,
        resources: Vec<String>,
    ) -> Result<(), AtlasError> {
        if resources.len() > 512 || resources.iter().any(|id| SessionId::parse(id).is_err()) {
            return Err(error(ErrorCode::InvalidInput));
        }
        let mut registry = lock(&self.shared.registry)?;
        let actor = self.actor(&registry, Some(authority))?;
        let actor_id = actor.id.clone();
        let session = registry
            .sessions
            .get_mut(id.as_str())
            .ok_or_else(|| error(ErrorCode::ExpiredSession))?;
        if session.actor != actor_id || session.target != *target {
            return Err(error(ErrorCode::Denied));
        }
        if session.expires <= Instant::now() {
            return Err(error(ErrorCode::ExpiredSession));
        }
        session.scope.resources = resources.into_iter().collect();
        Ok(())
    }
    pub fn validate_session(
        &self,
        authority: &Authority,
        id: &SessionId,
        target: &ServerTarget,
    ) -> Result<(), AtlasError> {
        let registry = lock(&self.shared.registry)?;
        let actor = self.actor(&registry, Some(authority))?;
        let session = registry
            .sessions
            .get(id.as_str())
            .ok_or_else(|| error(ErrorCode::ExpiredSession))?;
        if session.actor != actor.id || session.target != *target {
            return Err(error(ErrorCode::Denied));
        }
        if session.expires <= Instant::now() {
            return Err(error(ErrorCode::ExpiredSession));
        }
        Ok(())
    }
    pub fn close_session(&self, authority: &Authority, id: &SessionId) -> Result<(), AtlasError> {
        let mut registry = lock(&self.shared.registry)?;
        let actor = self.actor(&registry, Some(authority))?;
        if registry
            .sessions
            .get(id.as_str())
            .is_some_and(|s| s.actor != actor.id)
        {
            return Err(error(ErrorCode::Denied));
        }
        registry.sessions.remove(id.as_str());
        for job in registry
            .jobs
            .values()
            .filter(|job| job.session_id.as_ref() == Some(id))
        {
            job.cancelled.store(true, Ordering::Release);
        }
        Ok(())
    }
    fn validate_job(&self, authority: Option<&Authority>, job: &Job) -> Result<(), AtlasError> {
        let registry = lock(&self.shared.registry)?;
        self.validate_locked(&registry, authority, job)
    }
    fn validate_locked(
        &self,
        registry: &Registry,
        authority: Option<&Authority>,
        job: &Job,
    ) -> Result<(), AtlasError> {
        let actor = self.actor(registry, authority)?;
        if actor.id != job.actor {
            return Err(error(ErrorCode::Denied));
        }
        if let Some(id) = &job.session_id {
            let session = registry
                .sessions
                .get(id.as_str())
                .ok_or_else(|| error(ErrorCode::ExpiredSession))?;
            if session.actor != actor.id || session.target != job.target {
                return Err(error(ErrorCode::Denied));
            }
            if Instant::now() >= session.expires {
                return Err(error(ErrorCode::ExpiredSession));
            }
        } else if !matches!(
            job.plan,
            InspectionPlan::ConnectionCheck | InspectionPlan::SftpOpen
        ) {
            return Err(error(ErrorCode::Denied));
        }
        if job.cancelled.load(Ordering::Acquire) {
            return Err(error(ErrorCode::Cancelled));
        }
        if Instant::now() >= job.expires {
            return Err(error(ErrorCode::TimedOut));
        }
        Ok(())
    }
    pub fn prepare(
        &self,
        authority: Option<&Authority>,
        current_target: &ServerTarget,
        request: InspectionRequest,
    ) -> Result<OperationTicket, AtlasError> {
        let mut registry = lock(&self.shared.registry)?;
        let actor = self.actor(&registry, authority)?;
        let session = registry
            .sessions
            .get(request.session_id.as_str())
            .ok_or_else(|| error(ErrorCode::ExpiredSession))?;
        if session.actor != actor.id
            || session.target != *current_target
            || request.server_id != current_target.server_id
        {
            return Err(error(ErrorCode::Denied));
        }
        if session.expires <= Instant::now() {
            return Err(error(ErrorCode::ExpiredSession));
        }
        let plan = session.scope.plan(request.inspection)?;
        let duration = if matches!(plan, InspectionPlan::SftpTransfer { .. }) {
            Duration::from_secs(30 * 60)
        } else {
            self.duration
        };
        let expires = (Instant::now() + duration).min(session.expires);
        let job = Arc::new(Job {
            actor: actor.id.clone(),
            session_id: Some(request.session_id),
            target: current_target.clone(),
            plan,
            expires,
            phase: Mutex::new(Phase::Prepared),
            cancelled: AtomicBool::new(false),
        });
        Self::insert_job(&mut registry, job, expires)
    }
    /// Backend-only authentication bootstrap. It permits one fixed connection
    /// check, grants no SSH session, and cannot run protected inspections.
    pub fn prepare_connection_check(
        &self,
        authority: Option<&Authority>,
        target: &ServerTarget,
    ) -> Result<OperationTicket, AtlasError> {
        self.prepare_bootstrap(authority, target, InspectionPlan::ConnectionCheck)
    }
    pub fn prepare_sftp(
        &self,
        authority: Option<&Authority>,
        target: &ServerTarget,
    ) -> Result<OperationTicket, AtlasError> {
        self.prepare_bootstrap(authority, target, InspectionPlan::SftpOpen)
    }
    fn prepare_bootstrap(
        &self,
        authority: Option<&Authority>,
        target: &ServerTarget,
        plan: InspectionPlan,
    ) -> Result<OperationTicket, AtlasError> {
        let mut registry = lock(&self.shared.registry)?;
        let actor = self.actor(&registry, authority)?;
        let expires = Instant::now() + self.duration;
        let job = Arc::new(Job {
            actor: actor.id.clone(),
            session_id: None,
            target: target.clone(),
            plan,
            expires,
            phase: Mutex::new(Phase::Prepared),
            cancelled: AtomicBool::new(false),
        });
        Self::insert_job(&mut registry, job, expires)
    }
    fn insert_job(
        registry: &mut Registry,
        job: Arc<Job>,
        expires: Instant,
    ) -> Result<OperationTicket, AtlasError> {
        registry.jobs.retain(|_, old| {
            old.expires > Instant::now()
                || lock(&old.phase).is_ok_and(|phase| *phase == Phase::Running)
        });
        if registry.jobs.len() >= MAX_JOBS {
            return Err(error(ErrorCode::Conflict));
        }
        let operation_id = OperationId::generate();
        registry.jobs.insert(operation_id.as_str().into(), job);
        Ok(OperationTicket {
            operation_id,
            expires_at: (Utc::now() + expires.saturating_duration_since(Instant::now()))
                .to_rfc3339(),
        })
    }
    fn job(&self, authority: Option<&Authority>, id: &OperationId) -> Result<Arc<Job>, AtlasError> {
        let job = {
            let registry = lock(&self.shared.registry)?;
            self.actor(&registry, authority)?;
            registry
                .jobs
                .get(id.as_str())
                .cloned()
                .ok_or_else(|| error(ErrorCode::NotFound))?
        };
        self.validate_job(authority, &job)?;
        Ok(job)
    }
    pub fn cancel(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
    ) -> Result<(), AtlasError> {
        let registry = lock(&self.shared.registry)?;
        self.actor(&registry, authority)?;
        let job = registry
            .jobs
            .get(id.as_str())
            .ok_or_else(|| error(ErrorCode::NotFound))?;
        self.validate_locked(&registry, authority, job)?;
        if *lock(&job.phase)? == Phase::Finished {
            return Err(error(ErrorCode::Conflict));
        }
        job.cancelled.store(true, Ordering::Release);
        Ok(())
    }
    pub fn active_workers(&self) -> usize {
        self.shared.workers.load(Ordering::Acquire)
    }
    pub fn audit(&self) -> Result<Vec<OperationAudit>, AtlasError> {
        Ok(lock(&self.shared.registry)?.audit.iter().cloned().collect())
    }

    fn admit(&self, authority: Option<&Authority>, job: &Job) -> Result<(), AtlasError> {
        // Same registry→phase order as cancel and delivery. A previously
        // validated ticket cannot be admitted after cancel/revoke won the lock.
        let registry = lock(&self.shared.registry)?;
        self.validate_locked(&registry, authority, job)?;
        let mut phase = lock(&job.phase)?;
        if *phase != Phase::Prepared {
            return Err(error(ErrorCode::Conflict));
        }
        self.shared
            .workers
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_WORKERS).then_some(count + 1)
            })
            .map_err(|_| error(ErrorCode::Conflict))?;
        *phase = Phase::Running;
        Ok(())
    }

    /// Each backend-issued ticket runs at most once. This dispatcher accepts
    /// only read-only plans; confirmed mutations use their dedicated dispatcher.
    /// A hard return deadline does not authorise an executor to detach remote
    /// children: actual SSH/SFTP adapters must kill/close them on cancellation.
    pub fn execute<F>(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
        current_target: &ServerTarget,
        executor: F,
    ) -> Result<ProtectedText, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>
            + Send
            + 'static,
    {
        let job = self.job(authority, id)?;
        let session_id = job
            .session_id
            .clone()
            .ok_or_else(|| error(ErrorCode::Denied))?;
        if matches!(
            job.plan,
            InspectionPlan::ConnectionCheck
                | InspectionPlan::SftpOpen
                | InspectionPlan::SftpMutation { .. }
                | InspectionPlan::SftpTransfer { .. }
                | InspectionPlan::ElevationCheck
                | InspectionPlan::ProtectedPlacement
                | InspectionPlan::WorkloadControl { .. }
        ) {
            return Err(error(ErrorCode::Denied));
        }
        let expires = job.expires;
        let text = self.execute_job(authority, id, current_target, job, executor)?;
        Ok(ProtectedText {
            session_id,
            operation_id: id.clone(),
            expires_at: (Utc::now() + expires.saturating_duration_since(Instant::now()))
                .to_rfc3339(),
            text,
        })
    }
    pub fn execute_connection_check<F>(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
        current_target: &ServerTarget,
        executor: F,
    ) -> Result<String, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>
            + Send
            + 'static,
    {
        let job = self.job(authority, id)?;
        if job.session_id.is_some() || !matches!(job.plan, InspectionPlan::ConnectionCheck) {
            return Err(error(ErrorCode::Denied));
        }
        self.execute_job(authority, id, current_target, job, executor)
    }
    /// Only the dedicated confirmed-mutation adapter can dispatch this plan.
    /// Failed delivery is not evidence that a remote write did not happen.
    pub fn execute_mutation<F>(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
        target: &ServerTarget,
        executor: F,
    ) -> Result<String, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>
            + Send
            + 'static,
    {
        let job = self.job(authority, id)?;
        if job.session_id.is_none() || !matches!(job.plan, InspectionPlan::SftpMutation { .. }) {
            return Err(error(ErrorCode::Denied));
        }
        self.execute_job(authority, id, target, job, executor)
    }
    pub fn execute_sftp_open<F>(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
        target: &ServerTarget,
        executor: F,
    ) -> Result<String, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>
            + Send
            + 'static,
    {
        let job = self.job(authority, id)?;
        if job.session_id.is_some() || !matches!(job.plan, InspectionPlan::SftpOpen) {
            return Err(error(ErrorCode::Denied));
        }
        self.execute_job(authority, id, target, job, executor)
    }
    /// Streaming content stays inside the trusted adapter, with bounded
    /// metadata output and separate per-request I/O deadlines.
    pub fn execute_transfer<F>(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
        target: &ServerTarget,
        executor: F,
    ) -> Result<String, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>
            + Send
            + 'static,
    {
        let job = self.job(authority, id)?;
        if job.session_id.is_none() || !matches!(job.plan, InspectionPlan::SftpTransfer { .. }) {
            return Err(error(ErrorCode::Denied));
        }
        self.execute_job(authority, id, target, job, executor)
    }
    pub fn execute_elevation<F>(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
        target: &ServerTarget,
        executor: F,
    ) -> Result<String, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>
            + Send
            + 'static,
    {
        let job = self.job(authority, id)?;
        if job.session_id.is_none()
            || !matches!(
                job.plan,
                InspectionPlan::ElevationCheck | InspectionPlan::ProtectedPlacement
            )
        {
            return Err(error(ErrorCode::Denied));
        }
        self.execute_job(authority, id, target, job, executor)
    }
    /// Dedicated one-shot dispatch for an explicitly reviewed, authenticated
    /// workload mutation. Read-only executors cannot accept this plan.
    pub fn execute_workload_control<F>(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
        target: &ServerTarget,
        executor: F,
    ) -> Result<String, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>
            + Send
            + 'static,
    {
        let job = self.job(authority, id)?;
        if job.session_id.is_none() || !matches!(job.plan, InspectionPlan::WorkloadControl { .. }) {
            return Err(error(ErrorCode::Denied));
        }
        self.execute_job(authority, id, target, job, executor)
    }
    fn execute_job<F>(
        &self,
        authority: Option<&Authority>,
        id: &OperationId,
        current_target: &ServerTarget,
        job: Arc<Job>,
        executor: F,
    ) -> Result<String, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>
            + Send
            + 'static,
    {
        if job.target != *current_target {
            return Err(error(ErrorCode::Denied));
        }
        self.admit(authority, &job)?;
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let context = ExecutionContext {
            broker: self.clone(),
            authority: authority
                .cloned()
                .ok_or_else(|| error(ErrorCode::Unauthenticated))?,
            job: job.clone(),
            output: String::new(),
            failure: None,
            consumed: 0,
        };
        let shared = self.shared.clone();
        let spawned = std::thread::Builder::new()
            .name("atlas-read-operation".into())
            .spawn(move || {
                struct Worker(Arc<Shared>);
                impl Drop for Worker {
                    fn drop(&mut self) {
                        self.0.workers.fetch_sub(1, Ordering::AcqRel);
                    }
                }
                let _worker = Worker(shared);
                let _ = send.send(context.run(executor));
            });
        if spawned.is_err() {
            self.shared.workers.fetch_sub(1, Ordering::AcqRel);
            *lock(&job.phase)? = Phase::Finished;
            return Err(error(ErrorCode::IoFailure));
        }
        let result = loop {
            if let Err(failure) = self.validate_job(authority, &job) {
                break Err(failure);
            }
            let wait = job
                .expires
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(10));
            match receive.recv_timeout(wait) {
                Ok(result) => break self.validate_job(authority, &job).and(result),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break Err(error(ErrorCode::IoFailure)),
            }
        };
        let mut registry = lock(&self.shared.registry)?;
        // Linearise result delivery with revoke/session invalidation. A revoke
        // that won this lock must not race a successful protected response.
        let result = self.validate_locked(&registry, authority, &job).and(result);
        if result.is_err() {
            job.cancelled.store(true, Ordering::Release);
        }
        *lock(&job.phase)? = Phase::Finished;
        registry.audit.push_back(OperationAudit {
            operation_id: id.clone(),
            server_id: job.target.server_id.clone(),
            failure: result.as_ref().err().map(|failure| failure.code),
        });
        if registry.audit.len() > 200 {
            registry.audit.pop_front();
        }
        result
    }
}

/// Executors can only emit through the bounded sink. Errors are sticky: an
/// executor cannot ignore a limit/cancellation error and return success.
pub struct ExecutionContext {
    broker: OperationBroker,
    authority: Authority,
    job: Arc<Job>,
    output: String,
    failure: Option<ErrorCode>,
    consumed: usize,
}
impl ExecutionContext {
    fn run<F>(mut self, executor: F) -> Result<String, AtlasError>
    where
        F: FnOnce(&InspectionPlan, &mut ExecutionContext) -> Result<(), AtlasError>,
    {
        // A worker may have waited to be scheduled after admission. Recheck
        // before calling the executor, as well as on writes and delivery.
        self.checkpoint()?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            executor(&self.job.plan.clone(), &mut self)
        }))
        .unwrap_or_else(|_| Err(error(ErrorCode::IoFailure)));
        result
            .and_then(|_| self.checkpoint())
            .and_then(|_| match self.failure {
                Some(code) => Err(error(code)),
                None => Ok(self.output),
            })
            .map_err(|failure| error(failure.code))
    }
    pub fn checkpoint(&self) -> Result<(), AtlasError> {
        self.broker.validate_job(Some(&self.authority), &self.job)
    }
    pub fn write(&mut self, text: &str) -> Result<(), AtlasError> {
        self.account_output(text.len())?;
        self.output.push_str(text);
        Ok(())
    }
    /// Account captured subprocess bytes without exposing them as a response.
    pub fn account_output(&mut self, bytes: usize) -> Result<(), AtlasError> {
        let result = self.checkpoint().and_then(|_| {
            if self.consumed.saturating_add(bytes) > self.broker.output_limit {
                return Err(error(ErrorCode::OutputLimit));
            }
            self.consumed += bytes;
            Ok(())
        });
        if let Err(failure) = &result {
            self.failure.get_or_insert(failure.code);
        }
        result
    }
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        Inspection::decl(config),
        InspectionRequest::decl(config),
        OperationTicket::decl(config),
    ]
    .into_iter()
    .map(|declaration| format!("export {declaration}\n"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared() -> (OperationBroker, Authority, Arc<Job>, OperationId) {
        let broker = OperationBroker::default();
        let caller = broker.issue_backend_authority().unwrap();
        let target =
            ServerTarget::new(ServerId::parse("race-host").unwrap(), "race.invalid").unwrap();
        let session = broker
            .register_authenticated_session(
                &caller,
                target.clone(),
                SessionScope::new(vec![], vec![]).unwrap(),
                Duration::from_secs(60),
            )
            .unwrap();
        let ticket = broker
            .prepare(
                Some(&caller),
                &target,
                InspectionRequest {
                    server_id: target.server_id.clone(),
                    session_id: session,
                    inspection: Inspection::SystemSummary {},
                },
            )
            .unwrap();
        let job = broker.job(Some(&caller), &ticket.operation_id).unwrap();
        (broker, caller, job, ticket.operation_id)
    }

    #[test]
    fn stale_validation_cannot_admit_after_acknowledged_cancel_or_revoke() {
        for revoke in [false, true] {
            let (broker, caller, job, id) = prepared();
            // Hold the already validated Job exactly as execute does before
            // admission, then let cancellation/revocation win the race.
            if revoke {
                broker.revoke(&caller).unwrap();
            } else {
                broker.cancel(Some(&caller), &id).unwrap();
            }
            assert!(broker.admit(Some(&caller), &job).is_err());
            assert_eq!(broker.active_workers(), 0);
            assert!(*lock(&job.phase).unwrap() == Phase::Prepared);
        }
    }

    #[test]
    fn delayed_worker_checks_revocation_before_invoking_executor() {
        let (broker, caller, job, _) = prepared();
        let context = ExecutionContext {
            broker: broker.clone(),
            authority: caller.clone(),
            job,
            output: String::new(),
            failure: None,
            consumed: 0,
        };
        broker.revoke(&caller).unwrap();
        let error = context
            .run(|_, _| panic!("revoked worker must never invoke executor"))
            .err()
            .unwrap();
        assert_eq!(error.code, ErrorCode::Denied);
    }

    #[test]
    fn terminal_open_uses_an_authenticated_session_ticket_and_audits_no_shell_data() {
        let broker = OperationBroker::default();
        let caller = broker.issue_backend_authority().unwrap();
        let target = ServerTarget::new(
            ServerId::parse("terminal-host").unwrap(),
            "terminal.invalid",
        )
        .unwrap();
        let session = broker
            .register_authenticated_session(
                &caller,
                target.clone(),
                SessionScope::new(vec![], vec![]).unwrap(),
                Duration::from_secs(60),
            )
            .unwrap();
        let request = InspectionRequest {
            server_id: target.server_id.clone(),
            session_id: session,
            inspection: Inspection::TerminalOpen {},
        };
        let ticket = broker
            .prepare(Some(&caller), &target, request.clone())
            .unwrap();
        let result = broker
            .execute(
                Some(&caller),
                &ticket.operation_id,
                &target,
                |plan, context| {
                    assert!(matches!(plan, InspectionPlan::TerminalOpen));
                    context.checkpoint()
                },
            )
            .unwrap();
        assert!(result.text.is_empty());
        assert!(broker.audit().unwrap().last().unwrap().failure.is_none());

        broker.close_session(&caller, &request.session_id).unwrap();
        assert_eq!(
            broker
                .prepare(Some(&caller), &target, request)
                .err()
                .unwrap()
                .code,
            ErrorCode::ExpiredSession
        );
    }

    #[test]
    fn workload_control_requires_scoped_identity_and_dedicated_one_shot_dispatch() {
        let broker = OperationBroker::default();
        let caller = broker.issue_backend_authority().unwrap();
        let target =
            ServerTarget::new(ServerId::parse("control-host").unwrap(), "control.invalid").unwrap();
        let session = broker
            .register_authenticated_session(
                &caller,
                target.clone(),
                SessionScope::new(vec![], vec!["wl1_allowed".into()]).unwrap(),
                Duration::from_secs(60),
            )
            .unwrap();
        let make_request = |workload_id: &str| InspectionRequest {
            server_id: target.server_id.clone(),
            session_id: session.clone(),
            inspection: Inspection::WorkloadControl {
                workload_id: workload_id.into(),
            },
        };

        assert_eq!(
            broker
                .prepare(Some(&caller), &target, make_request("wl1_other"))
                .err()
                .unwrap()
                .code,
            ErrorCode::Denied
        );

        let ticket = broker
            .prepare(Some(&caller), &target, make_request("wl1_allowed"))
            .unwrap();
        assert_eq!(
            broker
                .execute(Some(&caller), &ticket.operation_id, &target, |_, _| panic!(
                    "read-only dispatcher must reject a workload mutation"
                ),)
                .err()
                .unwrap()
                .code,
            ErrorCode::Denied
        );
        let receipt = broker
            .execute_workload_control(
                Some(&caller),
                &ticket.operation_id,
                &target,
                |plan, context| {
                    assert!(matches!(plan, InspectionPlan::WorkloadControl { workload_id } if workload_id == "wl1_allowed"));
                    context.write("observed")
                },
            )
            .unwrap();
        assert_eq!(receipt, "observed");
        assert!(
            broker
                .execute_workload_control(
                    Some(&caller),
                    &ticket.operation_id,
                    &target,
                    |_, _| panic!("an action ticket must be one-shot"),
                )
                .is_err()
        );
        assert!(
            broker
                .audit()
                .unwrap()
                .iter()
                .any(|item| { item.operation_id == ticket.operation_id && item.failure.is_none() })
        );
    }

    #[test]
    fn expired_workload_session_cannot_dispatch_action() {
        let broker = OperationBroker::default();
        let caller = broker.issue_backend_authority().unwrap();
        let target = ServerTarget::new(
            ServerId::parse("expired-control-host").unwrap(),
            "control.invalid",
        )
        .unwrap();
        let session = broker
            .register_authenticated_session(
                &caller,
                target.clone(),
                SessionScope::new(vec![], vec!["wl1_allowed".into()]).unwrap(),
                Duration::from_millis(500),
            )
            .unwrap();
        let ticket = broker
            .prepare(
                Some(&caller),
                &target,
                InspectionRequest {
                    server_id: target.server_id.clone(),
                    session_id: session,
                    inspection: Inspection::WorkloadControl {
                        workload_id: "wl1_allowed".into(),
                    },
                },
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(550));
        let error = broker
            .execute_workload_control(Some(&caller), &ticket.operation_id, &target, |_, _| {
                panic!("an expired session must not dispatch a workload action")
            })
            .err()
            .unwrap();
        assert_eq!(error.code, ErrorCode::ExpiredSession);
    }

    #[test]
    fn cancelling_an_in_flight_workload_ticket_cannot_replay_its_executor() {
        let broker = OperationBroker::new(Duration::from_secs(3), MAX_OUTPUT).unwrap();
        let caller = broker.issue_backend_authority().unwrap();
        let target =
            ServerTarget::new(ServerId::parse("cancel-host").unwrap(), "cancel.invalid").unwrap();
        let session = broker
            .register_authenticated_session(
                &caller,
                target.clone(),
                SessionScope::new(vec![], vec!["wl1_allowed".into()]).unwrap(),
                Duration::from_secs(30),
            )
            .unwrap();
        let ticket = broker
            .prepare(
                Some(&caller),
                &target,
                InspectionRequest {
                    server_id: target.server_id.clone(),
                    session_id: session,
                    inspection: Inspection::WorkloadControl {
                        workload_id: "wl1_allowed".into(),
                    },
                },
            )
            .unwrap();
        let starts = Arc::new(AtomicUsize::new(0));
        let worker_broker = broker.clone();
        let worker_caller = caller.clone();
        let worker_target = target.clone();
        let worker_ticket = ticket.operation_id.clone();
        let worker_starts = Arc::clone(&starts);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            worker_broker.execute_workload_control(
                Some(&worker_caller),
                &worker_ticket,
                &worker_target,
                move |_, context| {
                    worker_starts.fetch_add(1, Ordering::AcqRel);
                    let _ = started_tx.send(());
                    loop {
                        context.checkpoint()?;
                        std::thread::sleep(Duration::from_millis(2));
                    }
                },
            )
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        broker.cancel(Some(&caller), &ticket.operation_id).unwrap();
        assert_eq!(
            worker.join().unwrap().unwrap_err().code,
            ErrorCode::Cancelled
        );
        assert_eq!(starts.load(Ordering::Acquire), 1);
        assert_eq!(
            broker
                .execute_workload_control(
                    Some(&caller),
                    &ticket.operation_id,
                    &target,
                    |_, _| panic!("cancelled action ticket must not be replayed"),
                )
                .err()
                .unwrap()
                .code,
            ErrorCode::Cancelled
        );
        assert_eq!(starts.load(Ordering::Acquire), 1);
    }

    #[test]
    fn timed_out_workload_ticket_cannot_replay_its_executor() {
        let broker = OperationBroker::new(Duration::from_millis(100), MAX_OUTPUT).unwrap();
        let caller = broker.issue_backend_authority().unwrap();
        let target =
            ServerTarget::new(ServerId::parse("timeout-host").unwrap(), "timeout.invalid").unwrap();
        let session = broker
            .register_authenticated_session(
                &caller,
                target.clone(),
                SessionScope::new(vec![], vec!["wl1_allowed".into()]).unwrap(),
                Duration::from_secs(30),
            )
            .unwrap();
        let ticket = broker
            .prepare(
                Some(&caller),
                &target,
                InspectionRequest {
                    server_id: target.server_id.clone(),
                    session_id: session,
                    inspection: Inspection::WorkloadControl {
                        workload_id: "wl1_allowed".into(),
                    },
                },
            )
            .unwrap();
        let starts = Arc::new(AtomicUsize::new(0));
        let worker_starts = Arc::clone(&starts);
        let error = broker
            .execute_workload_control(
                Some(&caller),
                &ticket.operation_id,
                &target,
                move |_, context| {
                    worker_starts.fetch_add(1, Ordering::AcqRel);
                    loop {
                        context.checkpoint()?;
                        std::thread::sleep(Duration::from_millis(2));
                    }
                },
            )
            .err()
            .unwrap();
        assert_eq!(error.code, ErrorCode::TimedOut);
        assert_eq!(starts.load(Ordering::Acquire), 1);
        assert!(
            broker
                .execute_workload_control(
                    Some(&caller),
                    &ticket.operation_id,
                    &target,
                    |_, _| panic!("timed out action ticket must not be replayed"),
                )
                .is_err()
        );
        assert_eq!(starts.load(Ordering::Acquire), 1);
    }
}
