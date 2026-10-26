//! One reviewed, freshly authenticated elevated action. No reusable sudo grant.
use super::*;
use crate::ssh::conversation::Peer;
use zeroize::Zeroizing;
/// Backend-only transient input; never serialized, cloned or debug-printed.
pub struct ActionPassword(Zeroizing<String>);
impl From<String> for ActionPassword {
    fn from(value: String) -> Self {
        Self(Zeroizing::new(value))
    }
}

#[derive(Clone, Serialize, Deserialize, TS)]
pub struct ElevationPreview {
    pub operation_id: OperationId,
    pub expires_at: String,
    pub server: String,
    pub endpoint: String,
    pub action: String,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct ElevationReceipt {
    pub operation_id: OperationId,
    pub verified: bool,
}
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProtectedPlacementRequest {
    pub destination: String,
    pub owner: u32,
    pub group: u32,
    pub mode: u16,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct ProtectedPlacementStage {
    pub stage_id: String,
    pub name: String,
    pub size_bytes: String,
    pub sha256: Option<String>,
    pub staging_location: String,
    pub cleanup_needed: bool,
    pub failure: Option<ErrorCode>,
}
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct ProtectedPlacementReceipt {
    pub operation_id: OperationId,
    pub destination: String,
    pub owner: u32,
    pub group: u32,
    pub mode: String,
    pub size_bytes: String,
    pub sha256: String,
    pub cleanup_needed: bool,
    pub staging_location: Option<String>,
}
#[derive(Clone)]
pub(super) struct Placement {
    pub(super) stage_id: String,
    pub(super) stage_dir: Vec<u8>,
    pub(super) stage_path: Vec<u8>,
    pub(super) resource_id: String,
    pub(super) stage: ProtectedPlacementStage,
    pub(super) stage_owner: Option<u32>,
    pub(super) ready: bool,
    pub(super) applied: bool,
}
#[derive(Clone)]
pub(super) enum Action {
    Check,
    Placement {
        stage_id: String,
        request: ProtectedPlacementRequest,
    },
}
pub(super) struct Pending {
    ticket: OperationTicket,
    action: Action,
    consumed: bool,
}

fn validate_placement(request: &ProtectedPlacementRequest) -> Result<(), AtlasError> {
    let path = request.destination.as_str();
    if path.len() > 4096
        || !path.starts_with('/')
        || path == "/"
        || path.ends_with('/')
        || path.contains('\\')
        || path
            .chars()
            .any(|character| character.is_control() || is_bidi_control(character))
        || path
            .split('/')
            .skip(1)
            .any(|part| part.is_empty() || part == "." || part == "..")
        || request.mode > 0o777
    {
        return Err(error(ErrorCode::InvalidInput));
    }
    Ok(())
}
fn is_bidi_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'
            | '\u{202b}'
            | '\u{202c}'
            | '\u{202d}'
            | '\u{202e}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}
fn fresh_sudo(
    peer: &Peer,
    password: &Zeroizing<String>,
    context: &mut ExecutionContext,
    valid: &dyn Fn() -> Result<(), AtlasError>,
) -> Result<(), AtlasError> {
    let prompt = format!("[ATLAS_SUDO_{}]", token());
    let validation = peer.run(
        &["/usr/bin/sudo", "-k", "-S", "-p", &prompt, "-v"],
        Some((&prompt, password)),
        context,
        valid,
    )?;
    if !validation.prompted {
        return Err(error(ErrorCode::Denied));
    }
    if !validation.success {
        return Err(error(ErrorCode::Unauthenticated));
    }
    valid()?;
    context.checkpoint()
}
const PLACE_SCRIPT: &str = r#"set -eu
src=$1; dst=$2; uid=$3; gid=$4; mode=$5; expected=$6; marker=$7; stage_uid=$8
parent=${dst%/*}; [ -n "$parent" ] || parent=/
[ -f "$src" ] && [ ! -L "$src" ] || exit 74
[ "$(/usr/bin/realpath -e -- "$src")" = "$src" ] || exit 74
[ "$(/usr/bin/stat -c '%a:%u' -- "$src")" = "600:$stage_uid" ] || exit 74
actual=$(/usr/bin/sha256sum -- "$src"); actual=${actual%% *}
[ "$actual" = "$expected" ] || exit 74
[ -d "$parent" ] && [ ! -L "$parent" ] || exit 74
[ "$(/usr/bin/realpath -e -- "$parent")" = "$parent" ] || exit 74
[ "$(/usr/bin/stat -c %u -- "$parent")" = 0 ] || exit 74
parent_mode=$(/usr/bin/stat -c %a -- "$parent")
[ $((0$parent_mode & 0022)) -eq 0 ] || exit 74
[ ! -e "$dst" ] && [ ! -L "$dst" ] || exit 73
umask 077
tmp=$(/usr/bin/mktemp --tmpdir="$parent" -- .atlas-placement.XXXXXXXXXX)
published=0
cleanup() { rc=$?; if [ "$published" = 1 ]; then /usr/bin/rm -f -- "$dst"; fi; /usr/bin/rm -f -- "$tmp"; if [ "$rc" -eq 73 ]; then printf 'ATLAS_PLACEMENT_COLLISION\n'; elif [ "$rc" -eq 74 ]; then printf 'ATLAS_PLACEMENT_REJECTED\n'; fi; exit "$rc"; }
trap cleanup EXIT
trap 'exit 128' HUP INT TERM
/bin/cat -- "$src" > "$tmp"
actual=$(/usr/bin/sha256sum -- "$tmp"); actual=${actual%% *}
[ "$actual" = "$expected" ] || exit 74
[ ! -e "$dst" ] && [ ! -L "$dst" ] || exit 73
if ! /usr/bin/ln -T -- "$tmp" "$dst"; then
  if [ -e "$dst" ] || [ -L "$dst" ]; then exit 73; fi
  exit 74
fi
published=1
/usr/bin/chown -- "$uid:$gid" "$dst"
/usr/bin/chmod -- "$mode" "$dst"
[ "$(/usr/bin/stat -c '%u:%g:%a' -- "$dst")" = "$uid:$gid:$mode" ] || exit 74
/usr/bin/rm -f -- "$tmp"
published=0
trap - EXIT
printf '%s\n' "$marker""#;

#[derive(Serialize, Deserialize)]
struct PlacementCleanup {
    cleanup_needed: bool,
    staging_location: Option<String>,
}

impl FileSessions {
    pub fn preview_elevation(
        &self,
        target: &ServerTarget,
        id: &SessionId,
    ) -> Result<ElevationPreview, AtlasError> {
        let session = self.session(target, id)?;
        let mut pending = lock(&session.elevation)?;
        if pending.as_ref().is_some_and(|item| item.consumed) {
            return Err(error(ErrorCode::Conflict));
        }
        if let Some(old) = pending.take() {
            let _ = self
                .broker
                .cancel(Some(&self.authority), &old.ticket.operation_id);
        }
        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::ElevationCheck {},
            },
        )?;
        let preview = ElevationPreview {
            operation_id: ticket.operation_id.clone(),
            expires_at: ticket.expires_at.clone(),
            server: target.host().into(),
            endpoint: format!(
                "{}@{}:{}",
                session.profile.user, session.profile.hostname, session.profile.port
            ),
            action: "Check root identity without changing files or services".into(),
        };
        *pending = Some(Pending {
            ticket,
            action: Action::Check,
            consumed: false,
        });
        Ok(preview)
    }
    pub fn preview_protected_placement(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        request: ProtectedPlacementRequest,
    ) -> Result<ElevationPreview, AtlasError> {
        validate_placement(&request)?;
        let session = self.session(target, id)?;
        let stage = lock(&session.placement)?
            .as_ref()
            .filter(|stage| stage.ready && !stage.applied)
            .map(|stage| (stage.stage_id.clone(), stage.stage.clone()))
            .ok_or_else(|| error(ErrorCode::NotFound))?;
        let mut pending = lock(&session.elevation)?;
        if pending.as_ref().is_some_and(|item| item.consumed) {
            return Err(error(ErrorCode::Conflict));
        }
        if let Some(old) = pending.take() {
            let _ = self
                .broker
                .cancel(Some(&self.authority), &old.ticket.operation_id);
        }
        let ticket = self.broker.prepare(
            Some(&self.authority),
            target,
            InspectionRequest {
                server_id: target.server_id().clone(),
                session_id: id.clone(),
                inspection: Inspection::ProtectedPlacement {},
            },
        )?;
        let preview = ElevationPreview {
            operation_id: ticket.operation_id.clone(),
            expires_at: ticket.expires_at.clone(),
            server: target.host().into(),
            endpoint: format!(
                "{}@{}:{}",
                session.profile.user, session.profile.hostname, session.profile.port
            ),
            action: format!(
                "Place {} bytes ({}) at {} as {}:{} with mode {:04o}; the staged SHA-256 is verified and the destination will not be overwritten",
                stage.1.size_bytes,
                stage.1.sha256.as_deref().unwrap_or("unverified"),
                request.destination,
                request.owner,
                request.group,
                request.mode,
            ),
        };
        *pending = Some(Pending {
            ticket,
            action: Action::Placement {
                stage_id: stage.0,
                request,
            },
            consumed: false,
        });
        Ok(preview)
    }
    pub fn cancel_elevation(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        operation: &OperationId,
    ) -> Result<(), AtlasError> {
        let session = self.session(target, id)?;
        let mut pending = lock(&session.elevation)?;
        let intent = pending
            .as_ref()
            .filter(|item| item.ticket.operation_id == *operation)
            .ok_or_else(|| error(ErrorCode::Denied))?;
        self.broker.cancel(Some(&self.authority), operation)?;
        if !intent.consumed {
            *pending = None;
        }
        Ok(())
    }
    pub fn confirm_elevation(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        operation: &OperationId,
        password: impl Into<ActionPassword>,
    ) -> Result<ElevationReceipt, AtlasError> {
        let password = password.into().0;
        if password.is_empty() || password.len() > 1024 || password.contains(['\r', '\n', '\0']) {
            return Err(error(ErrorCode::InvalidInput));
        }
        let session = self.session(target, id)?;
        {
            let mut pending = lock(&session.elevation)?;
            let intent = pending
                .as_mut()
                .filter(|item| item.ticket.operation_id == *operation && !item.consumed)
                .ok_or_else(|| error(ErrorCode::Denied))?;
            if !matches!(&intent.action, Action::Check) {
                return Err(error(ErrorCode::Denied));
            }
            intent.consumed = true;
        }
        let worker = session.clone();
        let options = self.options.clone();
        let policy = self.policy;
        let (send, receive) = std::sync::mpsc::channel();
        struct Done(std::sync::mpsc::Sender<()>);
        impl Drop for Done {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let done = Done(send);
        let result = self.broker.execute_elevation(
            Some(&self.authority),
            operation,
            target,
            move |_, context| {
                let _done = done;
                let valid = || {
                    if worker.view_valid(policy) {
                        Ok(())
                    } else {
                        Err(error(ErrorCode::ExpiredSession))
                    }
                };
                valid()?;
                let peer = options.pin_peer(
                    &worker.profile,
                    &worker.fingerprint,
                    worker
                        .host_key
                        .as_ref()
                        .ok_or_else(|| error(ErrorCode::Denied))?,
                    context,
                )?;
                fresh_sudo(&peer, &password, context, &valid)?;
                let prompt = format!("[ATLAS_SUDO_{}]", token());
                let action = peer.run(
                    &[
                        "/usr/bin/sudo",
                        "-k",
                        "-S",
                        "-p",
                        &prompt,
                        "--",
                        "/usr/bin/id",
                        "-u",
                    ],
                    Some((&prompt, &password)),
                    context,
                    &valid,
                )?;
                if !action.success {
                    return Err(error(ErrorCode::Denied));
                }
                if action.stdout.trim() != "0" {
                    return Err(error(ErrorCode::Denied));
                }
                *lock(&worker.last_used)? = Instant::now();
                context.write("verified")
            },
        );
        let _ = receive.recv();
        {
            let mut pending = lock(&session.elevation)?;
            if pending
                .as_ref()
                .is_some_and(|item| item.ticket.operation_id == *operation)
            {
                *pending = None;
            }
        }
        if result.as_ref().err().is_some_and(|failure| {
            matches!(
                failure.code,
                ErrorCode::IoFailure
                    | ErrorCode::HostTrustFailure
                    | ErrorCode::TimedOut
                    | ErrorCode::OutputLimit
                    | ErrorCode::ExpiredSession
            )
        }) {
            let _ = self.close(id);
        }
        result?;
        self.session(target, id)?;
        Ok(ElevationReceipt {
            operation_id: operation.clone(),
            verified: true,
        })
    }
    pub fn confirm_protected_placement(
        &self,
        target: &ServerTarget,
        id: &SessionId,
        operation: &OperationId,
        password: impl Into<ActionPassword>,
    ) -> Result<ProtectedPlacementReceipt, AtlasError> {
        let password = password.into().0;
        if password.is_empty() || password.len() > 1024 || password.contains(['\r', '\n', '\0']) {
            return Err(error(ErrorCode::InvalidInput));
        }
        let session = self.session(target, id)?;
        let (stage_id, request) = {
            let mut pending = lock(&session.elevation)?;
            let intent = pending
                .as_mut()
                .filter(|item| item.ticket.operation_id == *operation && !item.consumed)
                .ok_or_else(|| error(ErrorCode::Denied))?;
            let Action::Placement { stage_id, request } = &intent.action else {
                return Err(error(ErrorCode::Denied));
            };
            intent.consumed = true;
            (stage_id.clone(), request.clone())
        };
        let stage = lock(&session.placement)?
            .as_ref()
            .filter(|stage| stage.stage_id == stage_id && !stage.applied)
            .cloned()
            .ok_or_else(|| error(ErrorCode::Conflict))?;
        validate_placement(&request)?;
        let worker = session.clone();
        let worker_stage = stage.clone();
        let worker_request = request.clone();
        let worker_stage_id = stage_id.clone();
        let options = self.options.clone();
        let policy = self.policy;
        let action_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_action_started = std::sync::Arc::clone(&action_started);
        let (send, receive) = std::sync::mpsc::channel();
        struct Done(std::sync::mpsc::Sender<()>);
        impl Drop for Done {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let done = Done(send);
        let result = self.broker.execute_elevation(
            Some(&self.authority),
            operation,
            target,
            move |_, context| {
                let _done = done;
                let valid = || {
                    if worker.view_valid(policy) {
                        Ok(())
                    } else {
                        Err(error(ErrorCode::ExpiredSession))
                    }
                };
                valid()?;
                let mut data = worker
                    .data
                    .try_lock()
                    .map_err(|_| error(ErrorCode::Conflict))?;
                let peer = options.pin_peer(
                    &worker.profile,
                    &worker.fingerprint,
                    worker
                        .host_key
                        .as_ref()
                        .ok_or_else(|| error(ErrorCode::Denied))?,
                    context,
                )?;
                fresh_sudo(&peer, &password, context, &valid)?;

                let stage_path = std::str::from_utf8(&worker_stage.stage_path)
                    .map_err(|_| error(ErrorCode::InvalidInput))?;
                let owner = worker_request.owner.to_string();
                let group = worker_request.group.to_string();
                let mode = format!("{:o}", worker_request.mode);
                let marker = format!("ATLAS_PLACEMENT_{}", OperationId::generate().as_str());
                let stage_owner = worker_stage
                    .stage_owner
                    .ok_or_else(|| error(ErrorCode::Denied))?
                    .to_string();
                let prompt = format!("[ATLAS_SUDO_{}]", token());
                let args = [
                    "/usr/bin/sudo",
                    "-k",
                    "-S",
                    "-p",
                    &prompt,
                    "--",
                    "/bin/sh",
                    "-c",
                    PLACE_SCRIPT,
                    "atlas-placement",
                    stage_path,
                    worker_request.destination.as_str(),
                    owner.as_str(),
                    group.as_str(),
                    mode.as_str(),
                    worker_stage
                        .stage
                        .sha256
                        .as_deref()
                        .ok_or_else(|| error(ErrorCode::Denied))?,
                    marker.as_str(),
                    stage_owner.as_str(),
                ];
                worker_action_started.store(true, Ordering::Release);
                let applied = peer.run(&args, Some((&prompt, &password)), context, &valid)?;
                if !applied.prompted {
                    return Err(error(ErrorCode::Denied));
                }
                if !applied.success {
                    return Err(match applied.exit_code {
                        Some(73) => error(ErrorCode::Conflict),
                        Some(74) => error(ErrorCode::Denied),
                        _ => error(ErrorCode::RemoteRejected),
                    });
                }
                if !applied.stdout.lines().any(|line| line == marker) {
                    return Err(error(ErrorCode::OutcomeUnknown));
                }
                {
                    let mut placement = lock(&worker.placement)?;
                    let current = placement
                        .as_mut()
                        .filter(|current| current.stage_id == worker_stage_id)
                        .ok_or_else(|| error(ErrorCode::OutcomeUnknown))?;
                    current.applied = true;
                }
                *lock(&worker.last_used)? = Instant::now();
                let cleanup =
                    transfers::cleanup_placement(&mut data.client, &worker_stage, context);
                let cleanup_needed = cleanup.is_err();
                let staging_location = if cleanup_needed {
                    Some(worker_stage.stage.staging_location.clone())
                } else {
                    lock(&worker.placement)?.take();
                    None
                };
                let serialized = serde_json::to_string(&PlacementCleanup {
                    cleanup_needed,
                    staging_location,
                })
                .map_err(|_| error(ErrorCode::IoFailure))?;
                context.write(&serialized)
            },
        );
        let _ = receive.recv();
        {
            let mut pending = lock(&session.elevation)?;
            if pending
                .as_ref()
                .is_some_and(|item| item.ticket.operation_id == *operation)
            {
                *pending = None;
            }
        }
        if result.as_ref().err().is_some_and(|failure| {
            matches!(
                failure.code,
                ErrorCode::IoFailure
                    | ErrorCode::HostTrustFailure
                    | ErrorCode::TimedOut
                    | ErrorCode::OutputLimit
                    | ErrorCode::ExpiredSession
            )
        }) {
            let _ = self.close(id);
        }
        let result = match result {
            Err(failure)
                if action_started.load(Ordering::Acquire)
                    && !matches!(
                        failure.code,
                        ErrorCode::Conflict
                            | ErrorCode::Denied
                            | ErrorCode::RemoteRejected
                            | ErrorCode::Unauthenticated
                    ) =>
            {
                let _ = self.close(id);
                return Err(error(ErrorCode::OutcomeUnknown));
            }
            result => result?,
        };
        let cleanup: PlacementCleanup =
            serde_json::from_str(&result).map_err(|_| error(ErrorCode::OutcomeUnknown))?;
        self.session(target, id)?;
        Ok(ProtectedPlacementReceipt {
            operation_id: operation.clone(),
            destination: request.destination,
            owner: request.owner,
            group: request.group,
            mode: format!("{:04o}", request.mode),
            size_bytes: stage.stage.size_bytes,
            sha256: stage
                .stage
                .sha256
                .ok_or_else(|| error(ErrorCode::OutcomeUnknown))?,
            cleanup_needed: cleanup.cleanup_needed,
            staging_location: cleanup.staging_location,
        })
    }
}
pub(super) fn typescript_contracts() -> String {
    let config = Config::default();
    [
        ElevationPreview::decl(&config),
        ElevationReceipt::decl(&config),
        ProtectedPlacementRequest::decl(&config),
        ProtectedPlacementStage::decl(&config),
        ProtectedPlacementReceipt::decl(&config),
    ]
    .map(|decl| format!("export {decl}\n"))
    .join("")
}
