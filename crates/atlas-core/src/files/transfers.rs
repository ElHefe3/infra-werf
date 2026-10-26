//! Bounded native streams. Local paths/grants originate only in backend file
//! selection, never in IPC arguments. Payloads never enter renderer or Store.
use super::mutations::{absent, directory, joined, name, parent};
use super::*;
use crate::sftp::WireChange;
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicU8;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Weak,
};
use tempfile::NamedTempFile;

const MAX_BYTES: u64 = 32 * 1024 * 1024 * 1024;
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TransferDirection {
    Upload,
    Download,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TransferState {
    Queued,
    Opening,
    Transferring,
    Verifying,
    Publishing,
    Cancelling,
    Complete,
    Cancelled,
    Failed,
    OutcomeUnknown,
}
impl TransferState {
    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Cancelled | Self::Failed | Self::OutcomeUnknown
        )
    }
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct TransferView {
    pub id: OperationId,
    pub server_id: ServerId,
    pub session_id: SessionId,
    pub direction: TransferDirection,
    pub state: TransferState,
    pub bytes: String,
    pub total: Option<String>,
    pub remote: Option<String>,
    pub local_name: Option<String>,
    pub verified: bool,
    pub checksum: Option<String>,
    pub cleanup_needed: bool,
    pub failure: Option<ErrorCode>,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct LocalSelection {
    pub grant_id: String,
    pub name: String,
    pub path_label: String,
    pub bytes: Option<String>,
    pub direction: TransferDirection,
}
pub struct LocalTransfer {
    source: LocalData,
    label: String,
}
enum LocalData {
    Upload {
        file: File,
        size: u64,
    },
    Download {
        temporary: Option<NamedTempFile>,
        destination: PathBuf,
    },
}
impl LocalTransfer {
    pub fn upload(path: &Path) -> Result<Self, AtlasError> {
        let label = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| error(ErrorCode::InvalidInput))?
            .to_owned();
        name(&label)?;
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        let file = options
            .open(path)
            .map_err(|_| error(ErrorCode::IoFailure))?;
        let metadata = file.metadata().map_err(|_| error(ErrorCode::IoFailure))?;
        if !metadata.is_file() || metadata.len() > MAX_BYTES {
            return Err(error(ErrorCode::InvalidInput));
        }
        Ok(Self {
            source: LocalData::Upload {
                file,
                size: metadata.len(),
            },
            label,
        })
    }
    pub fn download(path: &Path) -> Result<Self, AtlasError> {
        if path.exists() {
            return Err(error(ErrorCode::Conflict));
        }
        let folder = path
            .parent()
            .ok_or_else(|| error(ErrorCode::InvalidInput))?
            .canonicalize()
            .map_err(|_| error(ErrorCode::IoFailure))?;
        let filename = path
            .file_name()
            .ok_or_else(|| error(ErrorCode::InvalidInput))?;
        let label = filename
            .to_str()
            .ok_or_else(|| error(ErrorCode::InvalidInput))?
            .to_owned();
        let destination = folder.join(filename);
        let temporary = tempfile::Builder::new()
            .prefix(".atlas-download-")
            .suffix(".part")
            .tempfile_in(&folder)
            .map_err(|_| error(ErrorCode::IoFailure))?;
        Ok(Self {
            source: LocalData::Download {
                temporary: Some(temporary),
                destination,
            },
            label,
        })
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn size(&self) -> Option<u64> {
        match self.source {
            LocalData::Upload { size, .. } => Some(size),
            _ => None,
        }
    }
}
struct Record {
    view: Mutex<TransferView>,
    session: Weak<Session>,
    cancel: AtomicBool,
}
struct Task {
    local: LocalTransfer,
    resource: Resource,
    record: Arc<Record>,
    session: Arc<Session>,
    ticket: OperationTicket,
}
#[derive(Default)]
pub(super) struct Queue {
    records: Mutex<VecDeque<Arc<Record>>>,
    tasks: Mutex<VecDeque<Task>>,
    running: AtomicBool,
}
impl Queue {
    pub(super) fn scrub(&self, policy: SessionPolicy) {
        if let Ok(records) = self.records.lock() {
            for record in records.iter() {
                if !record
                    .session
                    .upgrade()
                    .is_some_and(|session| session.valid(policy))
                {
                    if let Ok(mut view) = record.view.lock() {
                        view.remote = None;
                        view.local_name = None;
                        view.checksum = None;
                    }
                }
            }
        }
    }
}
impl FileSessions {
    pub fn validate_view(&self, target: &ServerTarget, id: &SessionId) -> Result<(), AtlasError> {
        self.session(target, id).map(|_| ())
    }
    pub fn transfer_name(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        resource: &str,
    ) -> Result<String, AtlasError> {
        let session = self.session(target, id)?;
        let snapshot = lock(&session.snapshot)?;
        let source = snapshot
            .get(resource)
            .ok_or_else(|| error(ErrorCode::Denied))?;
        if source.kind != FileKind::File || !source.mutable {
            return Err(error(ErrorCode::Denied));
        }
        Ok(source
            .label
            .chars()
            .map(|c| {
                if "<>:\"/\\|?*".contains(c) || c.is_control() {
                    '_'
                } else {
                    c
                }
            })
            .collect())
    }
    pub fn enqueue_transfer(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        resource_id: &str,
        local: LocalTransfer,
    ) -> Result<TransferView, AtlasError> {
        SessionId::parse(resource_id)?;
        let session = self.session(target, id)?;
        let resource = lock(&session.snapshot)?
            .get(resource_id)
            .cloned()
            .ok_or_else(|| error(ErrorCode::Denied))?;
        let direction = match local.source {
            LocalData::Upload { .. } => TransferDirection::Upload,
            _ => TransferDirection::Download,
        };
        if (direction == TransferDirection::Upload && resource.kind != FileKind::Directory)
            || (direction == TransferDirection::Download
                && (resource.kind != FileKind::File || !resource.mutable))
        {
            return Err(error(ErrorCode::Denied));
        }
        let mut records = lock(&self.queue.records)?;
        while records.len() >= 16 {
            if records
                .front()
                .is_some_and(|record| record.view.lock().is_ok_and(|view| view.state.terminal()))
            {
                records.pop_front();
            } else {
                return Err(error(ErrorCode::Conflict));
            }
        }
        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::SftpTransfer {
                    resource_id: resource_id.into(),
                },
            },
        )?;
        let remote = if direction == TransferDirection::Upload {
            sftp::display(&joined(&resource.path, name(&local.label)?)?)
        } else {
            sftp::display(&resource.path)
        };
        let view = TransferView {
            id: ticket.operation_id.clone(),
            server_id: target.server_id().clone(),
            session_id: id.clone(),
            direction,
            state: TransferState::Queued,
            bytes: "0".into(),
            total: local.size().map(|size| size.to_string()),
            remote: Some(remote),
            local_name: Some(local.label.clone()),
            verified: false,
            checksum: None,
            cleanup_needed: false,
            failure: None,
        };
        let record = Arc::new(Record {
            view: Mutex::new(view.clone()),
            session: Arc::downgrade(&session),
            cancel: AtomicBool::new(false),
        });
        records.push_back(record.clone());
        drop(records);
        session.transfers.fetch_add(1, Ordering::AcqRel);
        lock(&self.queue.tasks)?.push_back(Task {
            local,
            resource,
            record,
            session,
            ticket,
        });
        if self
            .queue
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let service = self.clone();
            if std::thread::Builder::new()
                .name("atlas-transfer-queue".into())
                .spawn(move || service.pump())
                .is_err()
            {
                self.queue.running.store(false, Ordering::Release);
                let tasks = lock(&self.queue.tasks)?.drain(..).collect::<Vec<_>>();
                for task in tasks {
                    self.finish_task(&task, Err(error(ErrorCode::IoFailure)));
                }
                return Err(error(ErrorCode::IoFailure));
            }
        }
        Ok(view)
    }
    pub fn transfer_views(&self) -> Result<Vec<TransferView>, AtlasError> {
        lock(&self.queue.records)?
            .iter()
            .map(|record| {
                let mut view = lock(&record.view)?;
                if !record
                    .session
                    .upgrade()
                    .is_some_and(|session| session.valid(self.policy))
                {
                    view.remote = None;
                    view.local_name = None;
                    view.checksum = None;
                }
                Ok(view.clone())
            })
            .collect()
    }
    pub fn cancel_transfer(&self, id: &OperationId) -> Result<(), AtlasError> {
        let records = lock(&self.queue.records)?;
        let record = records
            .iter()
            .find(|record| record.view.lock().is_ok_and(|view| view.id == *id))
            .ok_or_else(|| error(ErrorCode::NotFound))?;
        let mut view = lock(&record.view)?;
        if view.state == TransferState::Publishing {
            return Err(error(ErrorCode::Conflict));
        }
        if !view.state.terminal() {
            record.cancel.store(true, Ordering::Release);
            view.state = TransferState::Cancelling;
        }
        Ok(())
    }
    fn pump(&self) {
        loop {
            let task = match self.queue.tasks.lock() {
                Ok(mut tasks) => tasks.pop_front(),
                Err(_) => None,
            };
            let Some(mut task) = task else {
                self.queue.running.store(false, Ordering::Release);
                if self.queue.tasks.lock().is_ok_and(|tasks| !tasks.is_empty())
                    && self
                        .queue
                        .running
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                {
                    continue;
                }
                break;
            };
            let record = task.record.clone();
            let session = task.session.clone();
            let policy = self.policy;
            let mut local = LocalTransfer {
                source: LocalData::Download {
                    temporary: None,
                    destination: PathBuf::new(),
                },
                label: String::new(),
            };
            std::mem::swap(&mut local, &mut task.local);
            let resource = task.resource.clone();
            let published = Arc::new(AtomicU8::new(0));
            let result_state = published.clone();
            // Broker cancellation can return before its native worker exits.
            // Keep queue state/admission alive until handles and staging settle.
            struct Done(std::sync::mpsc::Sender<()>);
            impl Drop for Done {
                fn drop(&mut self) {
                    let _ = self.0.send(());
                }
            }
            let (done_send, done_receive) = std::sync::mpsc::channel();
            let done = Done(done_send);
            let result = self
                .broker
                .execute_transfer(
                    Some(&self.authority),
                    &task.ticket.operation_id,
                    &task.session.target,
                    move |_, context| {
                        let _done = done;
                        check(&record, &session, policy, context)?;
                        let mut data = loop {
                            match session.data.try_lock() {
                                Ok(data) => break data,
                                Err(std::sync::TryLockError::WouldBlock) => {
                                    check(&record, &session, policy, context)?;
                                    std::thread::sleep(Duration::from_millis(10));
                                }
                                Err(_) => return Err(error(ErrorCode::IoFailure)),
                            }
                        };
                        set_state(&record, TransferState::Opening)?;
                        let outcome = match &mut local.source {
                            LocalData::Upload { file, size } => upload(
                                &mut data.client,
                                &resource.path,
                                &local.label,
                                file,
                                *size,
                                &record,
                                &session,
                                policy,
                                context,
                                &result_state,
                            ),
                            LocalData::Download {
                                temporary,
                                destination,
                            } => download(
                                &mut data.client,
                                &resource.path,
                                temporary,
                                destination,
                                &record,
                                &session,
                                policy,
                                context,
                                &result_state,
                            ),
                        };
                        if outcome.is_err() {
                            return outcome;
                        }
                        context.write("verified")
                    },
                )
                .map(|_| ());
            let _ = done_receive.recv();
            let result = if result.is_err() && matches!(published.load(Ordering::Acquire), 1 | 3) {
                Err(error(ErrorCode::OutcomeUnknown))
            } else {
                result
            };
            if !task.session.valid(self.policy)
                || result.as_ref().err().is_some_and(|failure| {
                    matches!(
                        failure.code,
                        ErrorCode::IoFailure
                            | ErrorCode::TimedOut
                            | ErrorCode::ExpiredSession
                            | ErrorCode::OutcomeUnknown
                            | ErrorCode::OutputLimit
                    )
                })
            {
                let _ = self.close(&task.session.id);
            }
            self.finish_task(&task, result);
        }
    }
    fn finish_task(&self, task: &Task, result: Result<(), AtlasError>) {
        if let Ok(mut view) = task.record.view.lock() {
            match result {
                Ok(()) => view.state = TransferState::Complete,
                Err(failure) => {
                    view.failure = Some(failure.code);
                    view.state = match failure.code {
                        ErrorCode::Cancelled => TransferState::Cancelled,
                        ErrorCode::OutcomeUnknown => TransferState::OutcomeUnknown,
                        _ => TransferState::Failed,
                    };
                }
            }
        }
        if task.session.transfers.fetch_sub(1, Ordering::AcqRel) == 1
            && !task.session.visible.load(Ordering::Acquire)
        {
            let _ = self.close(&task.session.id);
        }
        self.queue.scrub(self.policy);
    }
}
fn check(
    record: &Record,
    session: &Session,
    policy: SessionPolicy,
    context: &ExecutionContext,
) -> Result<(), AtlasError> {
    context.checkpoint()?;
    if !session.valid(policy) {
        return Err(error(ErrorCode::ExpiredSession));
    }
    if record.cancel.load(Ordering::Acquire) {
        return Err(error(ErrorCode::Cancelled));
    }
    Ok(())
}
fn set_state(record: &Record, state: TransferState) -> Result<(), AtlasError> {
    lock(&record.view)?.state = state;
    Ok(())
}
fn progress(record: &Record, session: &Session, bytes: u64) -> Result<(), AtlasError> {
    lock(&record.view)?.bytes = bytes.to_string();
    *lock(&session.last_used)? = Instant::now();
    Ok(())
}
fn digest(hash: Sha256) -> String {
    format!("{:x}", hash.finalize())
}
fn publication_gate(
    record: &Record,
    session: &Session,
    policy: SessionPolicy,
    context: &ExecutionContext,
) -> Result<(), AtlasError> {
    let mut view = lock(&record.view)?;
    check(record, session, policy, context)?;
    view.state = TransferState::Publishing;
    Ok(())
}

fn upload(
    client: &mut Client,
    folder: &[u8],
    label: &str,
    file: &mut File,
    total: u64,
    record: &Record,
    session: &Session,
    policy: SessionPolicy,
    context: &mut ExecutionContext,
    published: &AtomicU8,
) -> Result<(), AtlasError> {
    directory(client, folder, context)?;
    let destination = joined(folder, name(label)?)?;
    absent(client, &destination, context)?;
    let stage = joined(folder, format!(".atlas-upload-{}", token()).as_bytes())?;
    lock(&record.view)?.cleanup_needed = true;
    client.private_dir(&stage, context)?;
    let payload = joined(&stage, b"payload")?;
    let handle = match client.open_file(&payload, true, context) {
        Ok(handle) => handle,
        Err(failure) => {
            if matches!(
                failure.code,
                ErrorCode::Denied | ErrorCode::RemoteRejected | ErrorCode::Conflict
            ) && context.checkpoint().is_ok()
            {
                match client.mutate(&WireChange::Rmdir(stage.clone()), context) {
                    Ok(Ok(())) => lock(&record.view)?.cleanup_needed = false,
                    Err(_) => session.closed.store(true, Ordering::Release),
                    _ => {}
                }
            }
            return Err(failure);
        }
    };
    let mut write_open = true;
    let mut verification_handle = None;
    let result = (|| {
        set_state(record, TransferState::Transferring)?;
        let mut buffer = [0u8; 32768];
        let mut offset = 0u64;
        let mut hash = Sha256::new();
        loop {
            check(record, session, policy, context)?;
            let count = file
                .read(&mut buffer)
                .map_err(|_| error(ErrorCode::IoFailure))?;
            if count == 0 {
                break;
            }
            if offset + count as u64 > total {
                return Err(error(ErrorCode::Conflict));
            }
            client.write_file(&handle, offset, &buffer[..count], context)?;
            hash.update(&buffer[..count]);
            offset += count as u64;
            progress(record, session, offset)?;
        }
        if offset != total {
            return Err(error(ErrorCode::Conflict));
        }
        client.close_dir(&handle, context)?;
        write_open = false;
        set_state(record, TransferState::Verifying)?;
        let expected = digest(hash);
        let read_handle = client.open_file(&payload, false, context)?;
        verification_handle = Some(read_handle.clone());
        let mut verified = Sha256::new();
        let mut read = 0u64;
        while let Some(bytes) = client.read_file(&read_handle, read, context)? {
            check(record, session, policy, context)?;
            read += bytes.len() as u64;
            if read > total {
                return Err(error(ErrorCode::Conflict));
            }
            verified.update(&bytes);
            *lock(&session.last_used)? = Instant::now();
        }
        client.close_dir(&read_handle, context)?;
        verification_handle = None;
        if read != total || digest(verified) != expected {
            return Err(error(ErrorCode::Conflict));
        }
        check(record, session, policy, context)?;
        absent(client, &destination, context)?;
        publication_gate(record, session, policy, context)?;
        // Once final publication is attempted, a lost response is uncertain.
        published.store(1, Ordering::Release);
        let ack = client.mutate(&WireChange::Rename(payload.clone(), destination), context)?;
        published.store(if ack.is_ok() { 3 } else { 2 }, Ordering::Release);
        ack?;
        {
            let mut view = lock(&record.view)?;
            view.verified = true;
            view.checksum = Some(expected);
        }
        client.mutate(&WireChange::Rmdir(stage.clone()), context)??;
        lock(&record.view)?.cleanup_needed = false;
        Ok(())
    })();
    if result.as_ref().err().is_some_and(|failure| {
        matches!(
            failure.code,
            ErrorCode::Cancelled
                | ErrorCode::Conflict
                | ErrorCode::Denied
                | ErrorCode::RemoteRejected
                | ErrorCode::NotFound
        )
    }) && matches!(published.load(Ordering::Acquire), 0 | 2)
        && context.checkpoint().is_ok()
    {
        // Only our acknowledged private staging entries; never a user target.
        let cleanup = (|| -> Result<(), AtlasError> {
            if write_open {
                client.close_dir(&handle, context)?;
            }
            if let Some(handle) = verification_handle {
                client.close_dir(&handle, context)?;
            }
            client.mutate(&WireChange::Remove(payload), context)??;
            client.mutate(&WireChange::Rmdir(stage), context)??;
            Ok(())
        })();
        if cleanup.is_ok() {
            if let Ok(mut view) = record.view.lock() {
                view.cleanup_needed = false;
            }
        } else {
            session.closed.store(true, Ordering::Release);
        }
    }
    result
}
fn download(
    client: &mut Client,
    path: &[u8],
    temporary: &mut Option<NamedTempFile>,
    destination: &Path,
    record: &Record,
    session: &Session,
    policy: SessionPolicy,
    context: &mut ExecutionContext,
    published: &AtomicU8,
) -> Result<(), AtlasError> {
    directory(client, &parent(path)?, context)?;
    let original = client.stat(path, context)?;
    if kind(&original) != FileKind::File {
        return Err(error(ErrorCode::Denied));
    }
    let total = original
        .size
        .ok_or_else(|| error(ErrorCode::InvalidInput))?;
    if total > MAX_BYTES {
        return Err(error(ErrorCode::InvalidInput));
    }
    lock(&record.view)?.total = Some(total.to_string());
    let handle = client.open_file(path, false, context)?;
    let mut remote_open = true;
    let result = (|| {
        if client.fstat(&handle, context)? != original {
            return Err(error(ErrorCode::Conflict));
        }
        set_state(record, TransferState::Transferring)?;
        let mut offset = 0u64;
        let mut hash = Sha256::new();
        while let Some(bytes) = client.read_file(&handle, offset, context)? {
            check(record, session, policy, context)?;
            offset += bytes.len() as u64;
            if offset > total {
                return Err(error(ErrorCode::Conflict));
            }
            temporary
                .as_mut()
                .ok_or_else(|| error(ErrorCode::IoFailure))?
                .write_all(&bytes)
                .map_err(|_| error(ErrorCode::IoFailure))?;
            hash.update(&bytes);
            progress(record, session, offset)?;
        }
        if offset != total || client.fstat(&handle, context)? != original {
            return Err(error(ErrorCode::Conflict));
        }
        client.close_dir(&handle, context)?;
        remote_open = false;
        set_state(record, TransferState::Verifying)?;
        let stage = temporary
            .as_mut()
            .ok_or_else(|| error(ErrorCode::IoFailure))?;
        stage
            .flush()
            .and_then(|_| stage.as_file().sync_all())
            .map_err(|_| error(ErrorCode::IoFailure))?;
        let expected = digest(hash);
        stage
            .as_file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|_| error(ErrorCode::IoFailure))?;
        let mut verify = Sha256::new();
        let mut buffer = [0u8; 32768];
        let mut checked = 0u64;
        loop {
            check(record, session, policy, context)?;
            let count = stage
                .as_file_mut()
                .read(&mut buffer)
                .map_err(|_| error(ErrorCode::IoFailure))?;
            if count == 0 {
                break;
            }
            checked += count as u64;
            verify.update(&buffer[..count]);
        }
        if checked != total || digest(verify) != expected {
            return Err(error(ErrorCode::Conflict));
        }
        check(record, session, policy, context)?;
        publication_gate(record, session, policy, context)?;
        let stage = temporary
            .take()
            .ok_or_else(|| error(ErrorCode::IoFailure))?;
        stage
            .persist_noclobber(destination)
            .map_err(|_| error(ErrorCode::Conflict))?;
        published.store(3, Ordering::Release);
        let mut view = lock(&record.view)?;
        view.verified = true;
        view.checksum = Some(expected);
        Ok(())
    })();
    if remote_open
        && result.as_ref().err().is_some_and(|failure| {
            matches!(
                failure.code,
                ErrorCode::Cancelled
                    | ErrorCode::Conflict
                    | ErrorCode::Denied
                    | ErrorCode::RemoteRejected
            )
        })
        && context.checkpoint().is_ok()
    {
        if client.close_dir(&handle, context).is_err() {
            session.closed.store(true, Ordering::Release);
        }
    }
    result
}
pub(super) fn typescript_contracts() -> String {
    let config = Config::default();
    [
        TransferDirection::decl(&config),
        TransferState::decl(&config),
        TransferView::decl(&config),
        LocalSelection::decl(&config),
    ]
    .map(|decl| format!("export {decl}\n"))
    .join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_worker_checksum_cannot_restore_closed_session_details() {
        let broker = OperationBroker::default();
        let authority = broker.issue_backend_authority().unwrap();
        let service = FileSessions::new(
            broker,
            authority,
            SshOptions::system(),
            SessionPolicy::default(),
        );
        let record = Arc::new(Record {
            session: Weak::new(),
            cancel: AtomicBool::new(false),
            view: Mutex::new(TransferView {
                id: OperationId::generate(),
                server_id: ServerId::parse("fixture").unwrap(),
                session_id: SessionId::parse(&token()).unwrap(),
                direction: TransferDirection::Download,
                state: TransferState::Publishing,
                bytes: "0".into(),
                total: Some("0".into()),
                remote: None,
                local_name: None,
                verified: false,
                checksum: None,
                cleanup_needed: false,
                failure: None,
            }),
        });
        service
            .queue
            .records
            .lock()
            .unwrap()
            .push_back(record.clone());
        service.queue.scrub(service.policy);
        record.view.lock().unwrap().checksum = Some("late protected checksum".into());
        assert!(service.transfer_views().unwrap()[0].checksum.is_none());
    }
}
