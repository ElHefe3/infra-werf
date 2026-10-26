//! Preview/confirm boundary for one path-scoped SFTP mutation. Prepared intents
//! are ephemeral, single-use and bound to the same principal/session/target.
use super::*;
use crate::sftp::WireChange;
use std::sync::atomic::AtomicU8;

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FileChange {
    Mkdir {
        parent_id: String,
        name: String,
    },
    Rename {
        resource_id: String,
        name: String,
    },
    Move {
        resource_id: String,
        parent_id: String,
    },
    Delete {
        resource_id: String,
    },
}
impl FileChange {
    fn resource(&self) -> &str {
        match self {
            Self::Mkdir { parent_id, .. } => parent_id,
            Self::Rename { resource_id, .. }
            | Self::Move { resource_id, .. }
            | Self::Delete { resource_id } => resource_id,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    CreateDirectory,
    Rename,
    Move,
    Delete,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct FileMutationPreview {
    pub operation_id: OperationId,
    pub expires_at: String,
    pub kind: MutationKind,
    pub server: String,
    pub endpoint: String,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub source_kind: Option<FileKind>,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct FileMutationReceipt {
    pub operation_id: OperationId,
    pub kind: MutationKind,
    pub refresh: FileResource,
}
pub(super) struct Pending {
    preview: FileMutationPreview,
    wire: WireChange,
    source: Option<(Vec<u8>, Attrs)>,
    parents: Vec<Vec<u8>>,
    destination: Option<Vec<u8>>,
    expires: Instant,
}
fn parent(path: &[u8]) -> Result<Vec<u8>, AtlasError> {
    let position = path
        .iter()
        .rposition(|byte| *byte == b'/')
        .ok_or_else(|| error(ErrorCode::InvalidInput))?;
    if position == 0 {
        Ok(b"/".to_vec())
    } else {
        Ok(path[..position].to_vec())
    }
}
fn name(value: &str) -> Result<&[u8], AtlasError> {
    if value.is_empty()
        || value.len() > 255
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.chars().any(char::is_control)
    {
        return Err(error(ErrorCode::InvalidInput));
    }
    Ok(value.as_bytes())
}
fn joined(directory: &[u8], name: &[u8]) -> Result<Vec<u8>, AtlasError> {
    let mut path = directory.to_vec();
    if path.last() != Some(&b'/') {
        path.push(b'/');
    }
    path.extend(name);
    if path.len() > 4096 {
        return Err(error(ErrorCode::InvalidInput));
    }
    Ok(path)
}
fn directory(
    client: &mut Client,
    path: &[u8],
    context: &mut ExecutionContext,
) -> Result<(), AtlasError> {
    if client.canonical(path, context)? != path
        || kind(&client.stat(path, context)?) != FileKind::Directory
    {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(())
}
fn absent(
    client: &mut Client,
    path: &[u8],
    context: &mut ExecutionContext,
) -> Result<(), AtlasError> {
    match client.stat(path, context) {
        Err(failure) if failure.code == ErrorCode::NotFound => Ok(()),
        Ok(_) => Err(error(ErrorCode::Conflict)),
        Err(failure) => Err(failure),
    }
}
fn resource(data: &Data, id: &str) -> Result<Resource, AtlasError> {
    SessionId::parse(id)?;
    data.resources
        .get(id)
        .cloned()
        .ok_or_else(|| error(ErrorCode::Denied))
}
fn draft(
    data: &mut Data,
    change: FileChange,
    context: &mut ExecutionContext,
) -> Result<
    (
        WireChange,
        Option<(Vec<u8>, Attrs)>,
        Vec<Vec<u8>>,
        Option<Vec<u8>>,
        MutationKind,
    ),
    AtlasError,
> {
    let selected = resource(data, change.resource())?;
    if let FileChange::Mkdir { name: label, .. } = change {
        if selected.kind != FileKind::Directory {
            return Err(error(ErrorCode::Denied));
        }
        directory(&mut data.client, &selected.path, context)?;
        let destination = joined(&selected.path, name(&label)?)?;
        absent(&mut data.client, &destination, context)?;
        return Ok((
            WireChange::Mkdir(destination.clone()),
            None,
            vec![selected.path],
            Some(destination),
            MutationKind::CreateDirectory,
        ));
    }
    if !selected.mutable
        || selected.path == b"/"
        || !matches!(
            selected.kind,
            FileKind::File | FileKind::Directory | FileKind::Symlink
        )
    {
        return Err(error(ErrorCode::Denied));
    }
    let source_parent = parent(&selected.path)?;
    directory(&mut data.client, &source_parent, context)?;
    let attrs = data.client.stat(&selected.path, context)?;
    if kind(&attrs) != selected.kind {
        return Err(error(ErrorCode::Conflict));
    }
    let source = Some((selected.path.clone(), attrs));
    match change {
        FileChange::Delete { .. } => {
            let wire = if selected.kind == FileKind::Directory {
                WireChange::Rmdir(selected.path)
            } else {
                WireChange::Remove(selected.path)
            };
            Ok((
                wire,
                source,
                vec![source_parent],
                None,
                MutationKind::Delete,
            ))
        }
        FileChange::Rename { name: label, .. } => {
            let destination = joined(&source_parent, name(&label)?)?;
            absent(&mut data.client, &destination, context)?;
            Ok((
                WireChange::Rename(selected.path, destination.clone()),
                source,
                vec![source_parent],
                Some(destination),
                MutationKind::Rename,
            ))
        }
        FileChange::Move { parent_id, .. } => {
            let target = resource(data, &parent_id)?;
            if target.kind != FileKind::Directory {
                return Err(error(ErrorCode::Denied));
            }
            directory(&mut data.client, &target.path, context)?;
            if selected.kind == FileKind::Directory
                && (target.path == selected.path
                    || target
                        .path
                        .starts_with(&[selected.path.as_slice(), b"/"].concat()))
            {
                return Err(error(ErrorCode::InvalidInput));
            }
            let basename = selected
                .path
                .rsplit(|byte| *byte == b'/')
                .next()
                .ok_or_else(|| error(ErrorCode::InvalidInput))?;
            let destination = joined(&target.path, basename)?;
            absent(&mut data.client, &destination, context)?;
            Ok((
                WireChange::Rename(selected.path, destination.clone()),
                source,
                vec![source_parent, target.path],
                Some(destination),
                MutationKind::Move,
            ))
        }
        FileChange::Mkdir { .. } => Err(error(ErrorCode::InvalidInput)),
    }
}
fn retained(data: &mut Data) -> Result<FileResource, AtlasError> {
    let current = data
        .directory
        .as_mut()
        .ok_or_else(|| error(ErrorCode::Conflict))?;
    current.cursor = None;
    current.pending.clear();
    let refresh = current.resource.clone();
    data.pinned = None;
    data.resources.retain(|_, resource| !resource.mutable);
    Ok(refresh)
}
impl FileSessions {
    pub fn preview_change(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        change: FileChange,
    ) -> Result<FileMutationPreview, AtlasError> {
        SessionId::parse(change.resource())?;
        let session = self.session(target, id)?;
        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::SftpRead {
                    resource_id: change.resource().into(),
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
                if data.directory.is_none() {
                    return Err(error(ErrorCode::Conflict));
                }
                let primary = change.resource().to_owned();
                let (wire, source, parents, destination, kind) = draft(&mut data, change, context)?;
                if let Some(previous) = data.pending.take() {
                    let _ = broker.cancel(Some(&authority), &previous.preview.operation_id);
                }
                context.checkpoint()?;
                let ticket = broker.prepare(
                    Some(&authority),
                    &worker.target,
                    InspectionRequest {
                        server_id: worker.target.server_id().clone(),
                        session_id: worker.id.clone(),
                        inspection: Inspection::SftpMutation {
                            resource_id: primary,
                        },
                    },
                )?;
                let preview = FileMutationPreview {
                    operation_id: ticket.operation_id,
                    expires_at: ticket.expires_at,
                    kind,
                    server: worker.target.host().into(),
                    endpoint: format!(
                        "{}@{}:{}",
                        data.client.profile.user,
                        data.client.profile.hostname,
                        data.client.profile.port
                    ),
                    source: source.as_ref().map(|(path, _)| sftp::display(path)),
                    destination: destination.as_ref().map(|path| sftp::display(path)),
                    source_kind: source.as_ref().map(|(_, attrs)| super::kind(attrs)),
                };
                data.pending = Some(Pending {
                    preview: preview.clone(),
                    wire,
                    source,
                    parents,
                    destination,
                    expires: Instant::now() + Duration::from_secs(30),
                });
                context.write(
                    &serde_json::to_string(&preview).map_err(|_| error(ErrorCode::IoFailure))?,
                )?;
                *lock(&worker.last_used)? = Instant::now();
                Ok(())
            },
        );
        match result {
            Ok(text) => {
                self.session(target, id)?;
                serde_json::from_str(&text.text).map_err(|_| error(ErrorCode::IoFailure))
            }
            Err(failure) => {
                self.close_on_failure(id, &failure);
                Err(failure)
            }
        }
    }
    pub fn discard_change(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        operation: &OperationId,
    ) -> Result<(), AtlasError> {
        let session = self.session(target, id)?;
        let mut data = session
            .data
            .try_lock()
            .map_err(|_| error(ErrorCode::Conflict))?;
        if data.pending.as_ref().map(|plan| &plan.preview.operation_id) != Some(operation) {
            return Err(error(ErrorCode::NotFound));
        }
        data.pending = None;
        match self.broker.cancel(Some(&self.authority), operation) {
            Err(failure)
                if matches!(
                    failure.code,
                    ErrorCode::TimedOut | ErrorCode::ExpiredSession
                ) =>
            {
                Ok(())
            }
            other => other,
        }
    }
    pub fn commit_change(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        operation: &OperationId,
    ) -> Result<FileMutationReceipt, AtlasError> {
        let session = self.session(target, id)?;
        let pending = {
            let mut data = session
                .data
                .try_lock()
                .map_err(|_| error(ErrorCode::Conflict))?;
            if data.pending.as_ref().map(|plan| &plan.preview.operation_id) != Some(operation) {
                return Err(error(ErrorCode::NotFound));
            }
            data.pending
                .take()
                .ok_or_else(|| error(ErrorCode::NotFound))?
        };
        if pending.expires <= Instant::now() {
            return Err(error(ErrorCode::TimedOut));
        }
        // Publish acknowledgement and success in one transition. Separate
        // flags could briefly expose "acknowledged but not yet applied" after
        // a success response while the caller is handling revocation/timeout.
        let phase = Arc::new(AtomicU8::new(0)); // 0 not sent, 1 sent, 2 rejected, 3 applied
        let progress = phase.clone();
        let worker = session.clone();
        let broker = self.broker.clone();
        let authority = self.authority.clone();
        let policy = self.policy;
        let result = self.broker.execute_mutation(
            Some(&self.authority),
            operation,
            target,
            move |_, context| {
                if !worker.valid(policy) {
                    return Err(error(ErrorCode::ExpiredSession));
                }
                let mut data = worker
                    .data
                    .try_lock()
                    .map_err(|_| error(ErrorCode::Conflict))?;
                for path in &pending.parents {
                    directory(&mut data.client, path, context)?;
                }
                if let Some((path, expected)) = &pending.source {
                    if data.client.stat(path, context)? != *expected {
                        return Err(error(ErrorCode::Conflict));
                    }
                }
                if let Some(path) = &pending.destination {
                    absent(&mut data.client, path, context)?;
                }
                if !worker.valid(policy) {
                    return Err(error(ErrorCode::ExpiredSession));
                }
                context.checkpoint()?;
                progress.store(1, Ordering::Release);
                let outcome = data.client.mutate(&pending.wire, context)?;
                progress.store(if outcome.is_ok() { 3 } else { 2 }, Ordering::Release);
                let refresh = retained(&mut data)?;
                broker.set_resources(
                    &authority,
                    &worker.id,
                    &worker.target,
                    data.resources.keys().cloned().collect(),
                )?;
                outcome?;
                let result = FileMutationReceipt {
                    operation_id: pending.preview.operation_id,
                    kind: pending.preview.kind,
                    refresh,
                };
                context.write(
                    &serde_json::to_string(&result).map_err(|_| error(ErrorCode::IoFailure))?,
                )?;
                *lock(&worker.last_used)? = Instant::now();
                Ok(())
            },
        );
        match result {
            Ok(text) => {
                if !session.valid(self.policy) {
                    let _ = self.close(id);
                    return Err(error(ErrorCode::OutcomeUnknown));
                }
                serde_json::from_str(&text).map_err(|_| error(ErrorCode::OutcomeUnknown))
            }
            Err(failure) => {
                if matches!(phase.load(Ordering::Acquire), 1 | 3) {
                    let _ = self.close(id);
                    return Err(error(ErrorCode::OutcomeUnknown));
                }
                self.close_on_failure(id, &failure);
                Err(failure)
            }
        }
    }
    fn close_on_failure(&self, id: &SessionId, failure: &AtlasError) {
        if !matches!(
            failure.code,
            ErrorCode::Denied
                | ErrorCode::NotFound
                | ErrorCode::InvalidInput
                | ErrorCode::Conflict
                | ErrorCode::RemoteRejected
        ) {
            let _ = self.close(id);
        }
    }
}
pub(super) fn typescript_contracts() -> String {
    let config = Config::default();
    [
        FileChange::decl(&config),
        MutationKind::decl(&config),
        FileMutationPreview::decl(&config),
        FileMutationReceipt::decl(&config),
    ]
    .map(|declaration| format!("export {declaration}\n"))
    .join("")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_names_are_literal_single_components_and_paths_stay_bounded() {
        for invalid in ["", ".", "..", "../escape", "a/b", "x\0y", "line\nname"] {
            assert!(name(invalid).is_err());
        }
        assert_eq!(
            name(";$(echo x) 日本語 ").unwrap(),
            ";$(echo x) 日本語 ".as_bytes()
        );
        assert!(joined(&vec![b'x'; 4090], b"long name").is_err());
        assert_eq!(parent(b"/file").unwrap(), b"/");
    }
}
