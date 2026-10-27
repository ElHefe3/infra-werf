//! Typed identities and bounded transient inspection for discovered workloads.
//! The fixed remote program accepts only identities issued from saved inventory.
use crate::{
    AtlasError, BaselineInventory, CONTRACT_VERSION, ContainerSummary, ErrorCode, ServerId,
    ServiceSummary,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::{Config, TS};

pub const MAX_WORKLOAD_LOG_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadKind {
    Service,
    Container,
}

impl WorkloadKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::Container => "container",
        }
    }
}

/// Read-only identity bound to one host and runtime scope. `id` is stable for
/// the same host/scope/name tuple and carries no authority by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkloadIdentity {
    pub id: String,
    pub kind: WorkloadKind,
    pub scope: String,
    pub runtime: String,
    pub name: String,
    pub reported_state: String,
    pub reported_detail: String,
    pub supported: bool,
}

impl WorkloadIdentity {
    fn new(
        server_id: &ServerId,
        kind: WorkloadKind,
        scope: &str,
        runtime: &str,
        name: &str,
        reported_state: &str,
        reported_detail: &str,
        supported: bool,
    ) -> Self {
        let mut digest = Sha256::new();
        for part in [server_id.as_str(), kind.as_str(), scope, runtime, name] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part.as_bytes());
        }
        let mut id = String::from("wl1_");
        for byte in digest.finalize() {
            use std::fmt::Write as _;
            let _ = write!(id, "{byte:02x}");
        }
        Self {
            id,
            kind,
            scope: scope.to_owned(),
            runtime: runtime.to_owned(),
            name: name.to_owned(),
            reported_state: reported_state.to_owned(),
            reported_detail: reported_detail.to_owned(),
            supported,
        }
    }

    pub fn from_inventory(
        server_id: &ServerId,
        inventory: Option<&BaselineInventory>,
    ) -> Vec<Self> {
        let Some(inventory) = inventory else {
            return Vec::new();
        };
        let services = inventory
            .services
            .iter()
            .map(|service| Self::service(server_id, service));
        let containers = inventory
            .containers
            .iter()
            .map(|container| Self::container(server_id, container));
        services.chain(containers).collect()
    }

    pub fn for_authenticated_session(
        server_id: &ServerId,
        target_host: &str,
        source_binding: &str,
        inventory: Option<&BaselineInventory>,
    ) -> Vec<Self> {
        inventory
            .filter(|inventory| {
                inventory.source_host.as_deref() == Some(target_host)
                    && inventory.source_binding.as_deref() == Some(source_binding)
            })
            .map(|inventory| Self::from_inventory(server_id, Some(inventory)))
            .unwrap_or_default()
    }

    fn service(server_id: &ServerId, service: &ServiceSummary) -> Self {
        let supported =
            matches!(service.scope.as_str(), "system" | "user") && valid_unit_name(&service.name);
        Self::new(
            server_id,
            WorkloadKind::Service,
            &service.scope,
            "systemd",
            &service.name,
            &service.state,
            &service.substate,
            supported,
        )
    }

    fn container(server_id: &ServerId, container: &ContainerSummary) -> Self {
        let supported = matches!(
            (container.scope.as_str(), container.engine.as_str()),
            ("user", "podman") | ("system", "docker")
        ) && valid_container_name(&container.name);
        Self::new(
            server_id,
            WorkloadKind::Container,
            &container.scope,
            &container.engine,
            &container.name,
            &container.state,
            &container.image,
            supported,
        )
    }
}

fn valid_unit_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() > 256
        || !name.ends_with(".service")
        || bytes
            .first()
            .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
    {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\\' {
            if bytes.get(index + 1) != Some(&b'x')
                || !bytes.get(index + 2).is_some_and(u8::is_ascii_hexdigit)
                || !bytes.get(index + 3).is_some_and(u8::is_ascii_hexdigit)
            {
                return false;
            }
            index += 4;
        } else if byte.is_ascii_alphanumeric() || b"_.:@-".contains(&byte) {
            index += 1;
        } else {
            return false;
        }
    }
    true
}

fn valid_container_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() <= 255
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(byte))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadState {
    Running,
    Stopped,
    PermissionDenied,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadLogState {
    Available,
    PermissionDenied,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadAction {
    Start,
    Stop,
    Restart,
}

impl WorkloadAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }

    fn expected_state(self) -> WorkloadState {
        match self {
            Self::Start | Self::Restart => WorkloadState::Running,
            Self::Stop => WorkloadState::Stopped,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadActionOutcome {
    Completed,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkloadControlPreview {
    pub contract_version: u32,
    pub operation_id: crate::OperationId,
    pub expires_at: String,
    pub server: String,
    pub endpoint: String,
    pub workload: WorkloadIdentity,
    pub action: WorkloadAction,
    pub requires_sudo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkloadControlReceipt {
    pub contract_version: u32,
    pub operation_id: crate::OperationId,
    pub workload_id: String,
    pub action: WorkloadAction,
    pub outcome: WorkloadActionOutcome,
    pub observed_state: WorkloadState,
    pub observed_at: String,
    pub message: String,
}

pub fn workload_control_requires_sudo(identity: &WorkloadIdentity) -> bool {
    matches!(
        (
            identity.kind,
            identity.scope.as_str(),
            identity.runtime.as_str()
        ),
        (WorkloadKind::Service, "system", "systemd")
            | (WorkloadKind::Container, "system", "docker")
    )
}

pub fn parse_workload_control(
    output: &str,
    operation_id: &crate::OperationId,
    identity: &WorkloadIdentity,
    action: WorkloadAction,
) -> Result<WorkloadControlReceipt, AtlasError> {
    let mut version = false;
    let mut exit = None;
    let mut state = None;
    let mut oneshot_completed = false;
    for line in output.lines() {
        if line == "ATLAS_CONTROL_V1" {
            if version {
                return Err(AtlasError::new(ErrorCode::IoFailure));
            }
            version = true;
        } else if let Some(value) = line.strip_prefix("ACTION_EXIT\t") {
            if exit.is_some()
                || value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(AtlasError::new(ErrorCode::IoFailure));
            }
            exit = value.parse::<u8>().ok();
        } else if let Some(value) = line.strip_prefix("STATE\t") {
            if state.is_some() {
                return Err(AtlasError::new(ErrorCode::IoFailure));
            }
            state = Some(match value {
                "running" => WorkloadState::Running,
                "stopped" => WorkloadState::Stopped,
                "oneshot_completed" => {
                    oneshot_completed = true;
                    WorkloadState::Stopped
                }
                "permission_denied" => WorkloadState::PermissionDenied,
                "unavailable" => WorkloadState::Unavailable,
                _ => return Err(AtlasError::new(ErrorCode::IoFailure)),
            });
        }
    }
    if !version {
        return Err(AtlasError::new(ErrorCode::IoFailure));
    }
    let exit = exit.ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?;
    let observed_state = state.ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?;
    if oneshot_completed
        && (identity.kind != WorkloadKind::Service
            || identity.runtime != "systemd"
            || action == WorkloadAction::Stop)
    {
        return Err(AtlasError::new(ErrorCode::IoFailure));
    }
    let outcome = if exit == 0 && (observed_state == action.expected_state() || oneshot_completed) {
        WorkloadActionOutcome::Completed
    } else if exit != 0 && (observed_state == action.expected_state() || oneshot_completed) {
        // The desired state is visible, but the remote action's own result was
        // unsuccessful, so we cannot attribute that state to this request.
        WorkloadActionOutcome::Unknown
    } else if matches!(
        observed_state,
        WorkloadState::Running | WorkloadState::Stopped
    ) {
        WorkloadActionOutcome::Failed
    } else {
        WorkloadActionOutcome::Unknown
    };
    let message = match outcome {
        WorkloadActionOutcome::Completed if oneshot_completed => format!(
            "The {} command completed successfully; the one-shot service is inactive after its task finished.",
            action.as_str()
        ),
        WorkloadActionOutcome::Completed => format!(
            "{} was observed after the {} request.",
            if observed_state == WorkloadState::Running {
                "Running"
            } else {
                "Stopped"
            },
            action.as_str()
        ),
        WorkloadActionOutcome::Failed => format!(
            "The {} request did not reach the expected state; the workload is {}.",
            action.as_str(),
            if observed_state == WorkloadState::Running {
                "running"
            } else {
                "stopped"
            }
        ),
        WorkloadActionOutcome::Unknown => {
            format!(
                "The {} command returned exit code {exit}; the observed state is {}. Inspect before retrying.",
                action.as_str(),
                match observed_state {
                    WorkloadState::Running => "running",
                    WorkloadState::Stopped => "stopped",
                    WorkloadState::PermissionDenied => "permission denied",
                    WorkloadState::Unavailable => "unavailable",
                }
            )
        }
    };
    Ok(WorkloadControlReceipt {
        contract_version: CONTRACT_VERSION,
        operation_id: operation_id.clone(),
        workload_id: identity.id.clone(),
        action,
        outcome,
        observed_state,
        observed_at: Utc::now().to_rfc3339(),
        message,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkloadDetail {
    pub label: String,
    pub value: String,
}

/// Protected result. It is returned only to the active UI and is never stored
/// in overview inventory, telemetry, or operation audit metadata.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkloadInspection {
    pub contract_version: u32,
    pub workload_id: String,
    pub inspected_at: String,
    pub state: WorkloadState,
    pub details: Vec<WorkloadDetail>,
    pub logs_state: WorkloadLogState,
    pub logs: String,
    pub logs_truncated: bool,
}

impl WorkloadInspection {
    pub fn unsupported(workload_id: &str) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            workload_id: workload_id.to_owned(),
            inspected_at: Utc::now().to_rfc3339(),
            state: WorkloadState::Unavailable,
            details: vec![WorkloadDetail {
                label: "Runtime".into(),
                value: "This runtime and scope combination is not supported.".into(),
            }],
            logs_state: WorkloadLogState::Unavailable,
            logs: String::new(),
            logs_truncated: false,
        }
    }
}

fn decode_state(value: &str) -> Result<WorkloadState, AtlasError> {
    match value {
        "running" => Ok(WorkloadState::Running),
        "stopped" => Ok(WorkloadState::Stopped),
        "permission_denied" => Ok(WorkloadState::PermissionDenied),
        "unavailable" => Ok(WorkloadState::Unavailable),
        _ => Err(AtlasError::new(ErrorCode::IoFailure)),
    }
}

fn decode_logs_state(value: &str) -> Result<WorkloadLogState, AtlasError> {
    match value {
        "available" => Ok(WorkloadLogState::Available),
        "permission_denied" => Ok(WorkloadLogState::PermissionDenied),
        "unavailable" => Ok(WorkloadLogState::Unavailable),
        _ => Err(AtlasError::new(ErrorCode::IoFailure)),
    }
}

/// Parse the fixed response header; log bytes begin after the unique marker so
/// log lines that resemble protocol tokens remain data, never control.
pub fn parse_workload_inspection(
    output: &str,
    workload_id: &str,
) -> Result<WorkloadInspection, AtlasError> {
    if output.len() > 48 * 1024 || workload_id.len() > 68 {
        return Err(AtlasError::new(ErrorCode::OutputLimit));
    }
    let (header, logs) = output
        .split_once("LOGS_BEGIN\n")
        .ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?;
    if logs.contains('\0') {
        return Err(AtlasError::new(ErrorCode::OutputLimit));
    }
    let mut lines = header.lines();
    if lines.next() != Some("ATLAS_WORKLOAD_V1") {
        return Err(AtlasError::new(ErrorCode::IoFailure));
    }
    let mut state = None;
    let mut logs_state = None;
    let mut logs_truncated = None;
    let mut details = Vec::new();
    for line in lines {
        let fields = line.split('\t').collect::<Vec<_>>();
        match fields.as_slice() {
            ["STATE", value] if state.is_none() => state = Some(decode_state(value)?),
            ["LOG_STATE", value] if logs_state.is_none() => {
                logs_state = Some(decode_logs_state(value)?)
            }
            ["LOGS_TRUNCATED", "true"] if logs_truncated.is_none() => logs_truncated = Some(true),
            ["LOGS_TRUNCATED", "false"] if logs_truncated.is_none() => logs_truncated = Some(false),
            ["DETAIL", label, value]
                if details.len() < 8
                    && matches!(
                        *label,
                        "Load state"
                            | "Active state"
                            | "Substate"
                            | "Unit file state"
                            | "Container state"
                            | "Exit code"
                            | "Started"
                            | "Finished"
                    )
                    && value.len() <= 128
                    && !value.chars().any(char::is_control)
                    && !details
                        .iter()
                        .any(|detail: &WorkloadDetail| detail.label == *label) =>
            {
                details.push(WorkloadDetail {
                    label: (*label).to_owned(),
                    value: (*value).to_owned(),
                });
            }
            _ => return Err(AtlasError::new(ErrorCode::IoFailure)),
        }
    }
    let mut logs_truncated = logs_truncated.ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?;
    let logs = if logs_state == Some(WorkloadLogState::Available) {
        let mut start = logs.len().saturating_sub(MAX_WORKLOAD_LOG_BYTES);
        while !logs.is_char_boundary(start) {
            start += 1;
        }
        if start > 0 {
            logs_truncated = true;
        }
        logs[start..].to_owned()
    } else {
        String::new()
    };
    Ok(WorkloadInspection {
        contract_version: CONTRACT_VERSION,
        workload_id: workload_id.to_owned(),
        inspected_at: Utc::now().to_rfc3339(),
        state: state.ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?,
        details,
        logs_state: logs_state.ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?,
        logs,
        logs_truncated,
    })
}

/// Fixed, read-only workload inspector. The four positional identity fields
/// are separately shell-quoted by the authenticated SSH adapter.
pub const REMOTE_WORKLOAD_INSPECTION: &str = r##"
set -o pipefail
export LC_ALL=C LANG=C
system_docker() {
  unset DOCKER_HOST DOCKER_CONTEXT DOCKER_TLS_VERIFY DOCKER_CERT_PATH
  command docker --host unix:///var/run/docker.sock "$@"
}
kind="$1"; scope="$2"; runtime="$3"; name="$4"
state=unavailable; logs_state=unavailable; logs=; truncated=false
detail_labels=(); detail_values=()
denied() { [[ "${1,,}" =~ (permission[[:space:]]denied|access[[:space:]]denied|not[[:space:]]authorized|operation[[:space:]]not[[:space:]]permitted) ]]; }
safe() { local value="${1//$'\t'/ }"; value="${value//$'\n'/ }"; value="${value//$'\r'/ }"; printf '%s' "${value:0:128}" | tr -cd '[:alnum:]_.:+ -'; }
property() { local key="$1"; while IFS= read -r line; do [[ "$line" == "$key="* ]] && { printf '%s' "${line#*=}"; return; }; done <<< "$properties"; }
capture_logs() {
  result="$("$@" 2>&1 | tail -c 8193; pipeline_status="${PIPESTATUS[0]}"; printf '\001'; exit "$pipeline_status")"
  status=$?
  result="${result%$'\001'}"
}
read_logs() {
  local result status
  if [[ "$kind" == service ]]; then
    if ! command -v journalctl >/dev/null 2>&1; then logs_state=unavailable; return; fi
    if [[ "$scope" == user ]]; then capture_logs journalctl --no-pager --lines=50 --output=short --user-unit="$name"
    else capture_logs journalctl --no-pager --lines=50 --output=short --unit="$name"; fi
  elif [[ "$runtime" == podman ]]; then
    if ! command -v podman >/dev/null 2>&1; then logs_state=unavailable; return; fi
    capture_logs podman logs --tail=50 --timestamps -- "$name"
  else
    if ! command -v docker >/dev/null 2>&1; then logs_state=unavailable; return; fi
    capture_logs system_docker logs --tail=50 --timestamps -- "$name"
  fi
  if (( ${#result} > 8192 )); then result="${result:1}"; truncated=true; fi
  if (( status == 0 )); then logs_state=available; logs="$result"
  elif denied "$result"; then logs_state=permission_denied
  else logs_state=unavailable; fi
}
if [[ "$kind" == service && "$runtime" == systemd && ( "$scope" == system || "$scope" == user ) ]] && command -v systemctl >/dev/null 2>&1; then
  if [[ "$scope" == user ]]; then properties="$(systemctl --user show --no-pager --property=LoadState --property=ActiveState --property=SubState --property=UnitFileState -- "$name" 2>&1)"; status=$?
  else properties="$(systemctl --system show --no-pager --property=LoadState --property=ActiveState --property=SubState --property=UnitFileState -- "$name" 2>&1)"; status=$?; fi
  if (( status == 0 )); then
    load="$(safe "$(property LoadState)")"; active="$(safe "$(property ActiveState)")"; sub="$(safe "$(property SubState)")"; unit="$(safe "$(property UnitFileState)")"
    if [[ -n "$load" && "$load" != not-found && -n "$active" ]]; then
      [[ "$active" == active ]] && state=running
      [[ "$active" == inactive || "$active" == failed ]] && state=stopped
      if [[ "$state" != unavailable ]]; then
        detail_labels+=("Load state" "Active state" "Substate" "Unit file state"); detail_values+=("$load" "$active" "$sub" "$unit")
        read_logs
      fi
    fi
  elif denied "$properties"; then state=permission_denied; logs_state=permission_denied
  fi
elif [[ "$kind" == container && ( ( "$runtime" == podman && "$scope" == user ) || ( "$runtime" == docker && "$scope" == system ) ) ]] && command -v "$runtime" >/dev/null 2>&1; then
  template='{{.State.Status}}|{{.State.ExitCode}}|{{.State.StartedAt}}|{{.State.FinishedAt}}'
  if [[ "$runtime" == docker ]]; then
    properties="$(system_docker inspect --format "$template" -- "$name" 2>&1)"; status=$?
  else
    properties="$(podman inspect --format "$template" -- "$name" 2>&1)"; status=$?
  fi
  if (( status == 0 )); then
    IFS='|' read -r container_state exit_code started finished <<< "$properties"
    container_state="$(safe "$container_state")"; exit_code="$(safe "$exit_code")"; started="$(safe "$started")"; finished="$(safe "$finished")"
    case "$container_state" in running) state=running;; exited|created|paused|dead|restarting|removing) state=stopped;; esac
    if [[ "$state" != unavailable ]]; then
      detail_labels+=("Container state" "Exit code" "Started" "Finished"); detail_values+=("$container_state" "$exit_code" "$started" "$finished")
      read_logs
    fi
  elif denied "$properties"; then state=permission_denied; logs_state=permission_denied
  fi
fi
printf 'ATLAS_WORKLOAD_V1\nSTATE\t%s\n' "$state"
for ((index=0; index<${#detail_labels[@]}; index++)); do printf 'DETAIL\t%s\t%s\n' "${detail_labels[index]}" "${detail_values[index]}"; done
printf 'LOG_STATE\t%s\nLOGS_TRUNCATED\t%s\nLOGS_BEGIN\n' "$logs_state" "$truncated"
printf '%s' "$logs"
"##;

/// Fixed mutation program. It accepts only typed identities and verbs, reports
/// the command exit code plus a fresh post-action observation, and exposes no
/// arbitrary command, path, or caller-selected executable.
pub const REMOTE_WORKLOAD_CONTROL: &str = r##"
set -u
export LC_ALL=C LANG=C
system_docker() {
  unset DOCKER_HOST DOCKER_CONTEXT DOCKER_TLS_VERIFY DOCKER_CERT_PATH
  command docker --host unix:///var/run/docker.sock "$@"
}
kind="$1"; scope="$2"; runtime="$3"; action="$4"; name="$5"
case "$action" in start|stop|restart) ;; *) exit 64;; esac
before=unavailable
systemd_observe() {
  local manager="$1" value rc key item active="" unit_type="" result=""
  if [[ "$manager" == system ]]; then
    value="$(systemctl --system show --no-pager --property=ActiveState --property=Type --property=Result -- "$name" 2>&1)"; rc=$?
  else
    value="$(systemctl --user show --no-pager --property=ActiveState --property=Type --property=Result -- "$name" 2>&1)"; rc=$?
  fi
  if (( rc != 0 )); then [[ "${value,,}" =~ (permission[[:space:]]denied|not[[:space:]]authorized|operation[[:space:]]not[[:space:]]permitted) ]] && printf permission_denied || printf unavailable; return; fi
  while IFS='=' read -r key item; do
    case "$key" in ActiveState) active="$item";; Type) unit_type="$item";; Result) result="$item";; esac
  done <<< "$value"
  if [[ "$action" != stop && "$active" == inactive && "$unit_type" == oneshot && "$result" == success ]]; then printf oneshot_completed; return; fi
  case "$active" in active) printf running;; inactive|failed) printf stopped;; activating|deactivating|reloading) printf transitioning;; *) printf unavailable;; esac
}
if [[ "$kind" == service && "$runtime" == systemd && "$scope" == system ]]; then
  systemctl --system "$action" "$name" >/dev/null 2>&1; action_rc=$?
  observe() { systemd_observe system; }
elif [[ "$kind" == service && "$runtime" == systemd && "$scope" == user ]]; then
  systemctl --user "$action" "$name" >/dev/null 2>&1; action_rc=$?
  observe() { systemd_observe user; }
elif [[ "$kind" == container && "$scope" == user && "$runtime" == podman ]]; then
  podman "$action" "$name" >/dev/null 2>&1; action_rc=$?
  observe() {
    value="$(podman inspect --format '{{.State.Status}}' -- "$name" 2>&1)"; rc=$?
    if (( rc != 0 )); then [[ "${value,,}" =~ (permission[[:space:]]denied|not[[:space:]]authorized|operation[[:space:]]not[[:space:]]permitted) ]] && printf permission_denied || printf unavailable; return; fi
    case "$value" in running) printf running;; exited|created|stopped|dead) printf stopped;; restarting|stopping|paused|removing) printf transitioning;; *) printf unavailable;; esac
  }
elif [[ "$kind" == container && "$scope" == system && "$runtime" == docker ]]; then
  system_docker "$action" "$name" >/dev/null 2>&1; action_rc=$?
  observe() {
    value="$(system_docker inspect --format '{{.State.Status}}' -- "$name" 2>&1)"; rc=$?
    if (( rc != 0 )); then [[ "${value,,}" =~ (permission[[:space:]]denied|not[[:space:]]authorized|operation[[:space:]]not[[:space:]]permitted) ]] && printf permission_denied || printf unavailable; return; fi
    case "$value" in running) printf running;; exited|created|stopped|dead) printf stopped;; restarting|stopping|paused|removing) printf transitioning;; *) printf unavailable;; esac
  }
else
  exit 64
fi
state=unavailable
for ((attempt=0; attempt<20; attempt++)); do
  state="$(observe)"
  if [[ "$action" == stop && "$state" == stopped ]] || [[ "$action" != stop && "$state" == running ]]; then break; fi
  if [[ "$action" != stop && "$state" == oneshot_completed ]]; then break; fi
  [[ "$state" == permission_denied || "$state" == unavailable ]] && break
  sleep 0.25
done
[[ "$state" != transitioning ]] || state=unavailable
printf 'ATLAS_CONTROL_V1\nACTION_EXIT\t%s\nSTATE\t%s\n' "$action_rc" "$state"
"##;

pub fn typescript_contracts() -> String {
    let config = Config::default();
    [
        WorkloadKind::decl(&config),
        WorkloadIdentity::decl(&config),
        WorkloadState::decl(&config),
        WorkloadLogState::decl(&config),
        WorkloadAction::decl(&config),
        WorkloadActionOutcome::decl(&config),
        WorkloadControlPreview::decl(&config),
        WorkloadControlReceipt::decl(&config),
        WorkloadDetail::decl(&config),
        WorkloadInspection::decl(&config),
    ]
    .map(|declaration| format!("export {declaration}\n"))
    .join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory() -> BaselineInventory {
        BaselineInventory {
            hostname: "fixture-node.home".into(),
            collected_at: "2026-10-26T10:00:00Z".into(),
            source_host: Some("fixture-alias".into()),
            source_binding: Some("binding-one".into()),
            cpus: 4,
            memory_total_kib: 1024,
            services: vec![
                ServiceSummary {
                    scope: "system".into(),
                    name: "worker.service".into(),
                    state: "active".into(),
                    substate: "running".into(),
                },
                ServiceSummary {
                    scope: "user".into(),
                    name: "worker.service".into(),
                    state: "inactive".into(),
                    substate: "dead".into(),
                },
            ],
            containers: vec![
                ContainerSummary {
                    scope: "user".into(),
                    engine: "podman".into(),
                    name: "api".into(),
                    image: "example/api:1".into(),
                    state: "running".into(),
                },
                ContainerSummary {
                    scope: "system".into(),
                    engine: "docker".into(),
                    name: "api".into(),
                    image: "example/api:1".into(),
                    state: "running".into(),
                },
                ContainerSummary {
                    scope: "user".into(),
                    engine: "docker".into(),
                    name: "unsupported".into(),
                    image: "example/api:1".into(),
                    state: "running".into(),
                },
            ],
        }
    }

    #[test]
    fn workload_ids_bind_host_scope_runtime_and_name() {
        let server = ServerId::generate();
        let other = ServerId::generate();
        let values = WorkloadIdentity::from_inventory(&server, Some(&inventory()));
        assert_eq!(values.len(), 5);
        let system = &values[0];
        let user = &values[1];
        assert_ne!(system.id, user.id);
        assert_ne!(
            system.id,
            WorkloadIdentity::from_inventory(&other, Some(&inventory()))[0].id
        );
        assert!(values[2].supported); // user Podman
        assert!(values[3].supported); // system Docker
        assert!(!values[4].supported); // no runtime/scope fallback
        assert!(
            values
                .iter()
                .all(|workload| workload.id.starts_with("wl1_"))
        );
    }

    #[test]
    fn workload_control_requires_sudo_only_for_system_scoped_adapters() {
        let identities = WorkloadIdentity::from_inventory(
            &ServerId::parse("fixture-server").unwrap(),
            Some(&inventory()),
        );
        assert!(workload_control_requires_sudo(&identities[0])); // systemd system
        assert!(!workload_control_requires_sudo(&identities[1])); // systemd user
        assert!(!workload_control_requires_sudo(&identities[2])); // rootless Podman
        assert!(workload_control_requires_sudo(&identities[3])); // system Docker
        assert!(!workload_control_requires_sudo(&identities[4])); // unsupported identity
    }

    #[test]
    fn workload_control_receipt_uses_the_post_action_observation() {
        let identity = WorkloadIdentity::from_inventory(
            &ServerId::parse("fixture-server").unwrap(),
            Some(&inventory()),
        )
        .remove(0);
        let operation = crate::OperationId::generate();
        let completed = parse_workload_control(
            "ATLAS_CONTROL_V1\nACTION_EXIT\t0\nSTATE\trunning\n",
            &operation,
            &identity,
            WorkloadAction::Start,
        )
        .unwrap();
        assert_eq!(completed.outcome, WorkloadActionOutcome::Completed);
        assert_eq!(completed.observed_state, WorkloadState::Running);
        assert_eq!(completed.action, WorkloadAction::Start);

        let one_shot = WorkloadIdentity::new(
            &ServerId::parse("fixture-server").unwrap(),
            WorkloadKind::Service,
            "system",
            "systemd",
            "fixture-task.service",
            "inactive",
            "dead",
            true,
        );
        for action in [WorkloadAction::Start, WorkloadAction::Restart] {
            let receipt = parse_workload_control(
                "ATLAS_CONTROL_V1\nACTION_EXIT\t0\nSTATE\toneshot_completed\n",
                &operation,
                &one_shot,
                action,
            )
            .unwrap();
            assert_eq!(receipt.outcome, WorkloadActionOutcome::Completed);
            assert_eq!(receipt.observed_state, WorkloadState::Stopped);
            assert!(receipt.message.contains("one-shot service is inactive"));
        }
        assert!(
            parse_workload_control(
                "ATLAS_CONTROL_V1\nACTION_EXIT\t0\nSTATE\toneshot_completed\n",
                &operation,
                &one_shot,
                WorkloadAction::Stop,
            )
            .is_err()
        );

        let failed = parse_workload_control(
            "ATLAS_CONTROL_V1\nACTION_EXIT\t1\nSTATE\tstopped\n",
            &operation,
            &identity,
            WorkloadAction::Start,
        )
        .unwrap();
        assert_eq!(failed.outcome, WorkloadActionOutcome::Failed);
        assert_eq!(failed.observed_state, WorkloadState::Stopped);

        let unknown = parse_workload_control(
            "ATLAS_CONTROL_V1\nACTION_EXIT\t1\nSTATE\trunning\n",
            &operation,
            &identity,
            WorkloadAction::Start,
        )
        .unwrap();
        assert_eq!(unknown.outcome, WorkloadActionOutcome::Unknown);
        assert!(
            parse_workload_control(
                "ATLAS_CONTROL_V1\nACTION_EXIT\t0\nSTATE\tother\n",
                &operation,
                &identity,
                WorkloadAction::Start,
            )
            .is_err()
        );
    }

    #[test]
    fn every_system_docker_adapter_pins_the_same_local_socket() {
        for program in [
            crate::inventory::REMOTE_BASELINE_COMMAND,
            REMOTE_WORKLOAD_INSPECTION,
            REMOTE_WORKLOAD_CONTROL,
        ] {
            assert!(
                program.contains(
                    "unset DOCKER_HOST DOCKER_CONTEXT DOCKER_TLS_VERIFY DOCKER_CERT_PATH"
                )
            );
            assert!(program.contains("docker --host unix:///var/run/docker.sock"));
        }
        assert!(REMOTE_WORKLOAD_INSPECTION.contains("capture_logs system_docker logs"));
        assert!(REMOTE_WORKLOAD_CONTROL.contains("system_docker inspect"));
    }

    #[test]
    fn imported_or_differently_authenticated_inventory_does_not_grant_workload_ids() {
        let server = ServerId::generate();
        let saved = inventory();
        assert_eq!(
            WorkloadIdentity::for_authenticated_session(
                &server,
                "other-alias",
                "binding-one",
                Some(&saved),
            )
            .len(),
            0
        );
        let mut imported = saved.clone();
        imported.source_host = None;
        imported.source_binding = None;
        assert_eq!(
            WorkloadIdentity::for_authenticated_session(
                &server,
                "fixture-alias",
                "binding-one",
                Some(&imported),
            )
            .len(),
            0
        );
        assert_eq!(
            WorkloadIdentity::for_authenticated_session(
                &server,
                "fixture-alias",
                "different-user-or-key",
                Some(&saved),
            )
            .len(),
            0
        );
        assert_eq!(
            WorkloadIdentity::for_authenticated_session(
                &server,
                "fixture-alias",
                "binding-one",
                Some(&saved),
            )
            .len(),
            5
        );
    }

    #[test]
    fn workload_ids_require_the_resolved_ssh_profile_and_host_key() {
        let server = ServerId::generate();
        let profile = crate::ssh::SshProfile {
            alias: "fixture-alias".into(),
            hostname: "127.0.0.1".into(),
            user: "fixture".into(),
            port: 2222,
            identity_files: 1,
        };
        let fingerprint = "SHA256:fixture-key";
        let binding = crate::inventory::authenticated_source_binding(&profile, fingerprint);
        let mut saved = inventory();
        saved.source_binding = Some(binding.clone());
        assert_eq!(
            WorkloadIdentity::for_authenticated_session(
                &server,
                &profile.alias,
                &binding,
                Some(&saved),
            )
            .len(),
            5
        );

        let mut changed_profiles = vec![profile.clone(); 3];
        changed_profiles[0].hostname = "other-host.home".into();
        changed_profiles[1].user = "other-account".into();
        changed_profiles[2].port = 2223;
        for changed in changed_profiles {
            let changed_binding =
                crate::inventory::authenticated_source_binding(&changed, fingerprint);
            assert!(
                WorkloadIdentity::for_authenticated_session(
                    &server,
                    &changed.alias,
                    &changed_binding,
                    Some(&saved),
                )
                .is_empty()
            );
        }
        let changed_key =
            crate::inventory::authenticated_source_binding(&profile, "SHA256:other-key");
        assert!(
            WorkloadIdentity::for_authenticated_session(
                &server,
                &profile.alias,
                &changed_key,
                Some(&saved),
            )
            .is_empty()
        );
    }

    #[test]
    fn malformed_inventory_names_cannot_become_runtime_arguments() {
        let server = ServerId::generate();
        let service = ServiceSummary {
            scope: "system".into(),
            name: "--property=UnitFileState.service".into(),
            state: "active".into(),
            substate: "running".into(),
        };
        assert!(!WorkloadIdentity::service(&server, &service).supported);
        assert!(valid_unit_name("worker@2.service"));
        assert!(valid_unit_name(r"worker\x2dblue.service"));
        assert!(!valid_unit_name("worker.service;reboot"));

        let container = ContainerSummary {
            scope: "user".into(),
            engine: "podman".into(),
            name: "--volume=/etc".into(),
            image: "fixture".into(),
            state: "running".into(),
        };
        assert!(!WorkloadIdentity::container(&server, &container).supported);
        assert!(valid_container_name("api_2.1"));
    }

    #[test]
    fn parser_bounds_logs_and_keeps_protocol_looking_log_lines_as_data() {
        let id = "wl1_0000000000000000000000000000000000000000000000000000000000000000";
        let sample = format!(
            "ATLAS_WORKLOAD_V1\nSTATE\trunning\nDETAIL\tActive state\tactive\nLOG_STATE\tavailable\nLOGS_TRUNCATED\tfalse\nLOGS_BEGIN\nordinary\nLOGS_BEGIN\nlog data\n"
        );
        let parsed = parse_workload_inspection(&sample, id).unwrap();
        assert_eq!(parsed.workload_id, id);
        assert_eq!(parsed.state, WorkloadState::Running);
        assert_eq!(parsed.details[0].value, "active");
        assert_eq!(parsed.logs, "ordinary\nLOGS_BEGIN\nlog data\n");

        let oversized = format!(
            "ATLAS_WORKLOAD_V1\nSTATE\tstopped\nLOG_STATE\tavailable\nLOGS_TRUNCATED\ttrue\nLOGS_BEGIN\n{}",
            "x".repeat(MAX_WORKLOAD_LOG_BYTES + 1)
        );
        let parsed = parse_workload_inspection(&oversized, id).unwrap();
        assert_eq!(parsed.logs.len(), MAX_WORKLOAD_LOG_BYTES);
        assert!(parsed.logs_truncated);

        let raw = format!("{}ATLAS_TEST_LOG_TAIL", "界".repeat(2731)).into_bytes();
        let lossy = String::from_utf8_lossy(&raw[1..]).into_owned();
        assert!(lossy.len() > MAX_WORKLOAD_LOG_BYTES);
        let clipped_utf8 = format!(
            "ATLAS_WORKLOAD_V1\nSTATE\trunning\nLOG_STATE\tavailable\nLOGS_TRUNCATED\tfalse\nLOGS_BEGIN\n{lossy}"
        );
        let parsed = parse_workload_inspection(&clipped_utf8, id).unwrap();
        assert!(parsed.logs.len() <= MAX_WORKLOAD_LOG_BYTES);
        assert!(parsed.logs_truncated);
        assert!(parsed.logs.ends_with("ATLAS_TEST_LOG_TAIL"));
    }

    #[test]
    fn parser_returns_explicit_denial_and_unavailable_states_without_raw_errors() {
        let output = "ATLAS_WORKLOAD_V1\nSTATE\tpermission_denied\nLOG_STATE\tpermission_denied\nLOGS_TRUNCATED\tfalse\nLOGS_BEGIN\n";
        let result = parse_workload_inspection(output, "wl1_fixture").unwrap();
        assert_eq!(result.state, WorkloadState::PermissionDenied);
        assert_eq!(result.logs_state, WorkloadLogState::PermissionDenied);
        assert!(result.logs.is_empty());

        let unknown = "ATLAS_WORKLOAD_V1\nSTATE\tunavailable\nLOG_STATE\tunavailable\nLOGS_TRUNCATED\tfalse\nLOGS_BEGIN\n";
        assert_eq!(
            parse_workload_inspection(unknown, "wl1_fixture")
                .unwrap()
                .state,
            WorkloadState::Unavailable
        );
    }
}
