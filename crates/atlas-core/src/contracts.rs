use crate::{
    health::{HealthProbeConfig, HealthSnapshot, ProbeKind, ServiceHealth},
    validation,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::{Config, TS};

pub const CONTRACT_VERSION: u32 = 1;
pub const STALE_AFTER_SECONDS: i64 = 900;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    Unauthenticated,
    Denied,
    HostTrustFailure,
    ExpiredSession,
    IoFailure,
    NotFound,
    Conflict,
    IncompatibleVersion,
    MigrationFailed,
    Cancelled,
    TimedOut,
    OutputLimit,
    RemoteRejected,
    OutcomeUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AtlasError {
    pub code: ErrorCode,
    pub message: String,
}

impl AtlasError {
    pub fn new(code: ErrorCode) -> Self {
        let message = match code {
            ErrorCode::InvalidInput => "The supplied data is invalid.",
            ErrorCode::Unauthenticated => "Authentication is required.",
            ErrorCode::Denied => "This operation is not permitted.",
            ErrorCode::HostTrustFailure => "The server identity requires review.",
            ErrorCode::ExpiredSession => "The authenticated session has expired.",
            ErrorCode::IoFailure => "The input/output operation failed.",
            ErrorCode::NotFound => "The requested record does not exist.",
            ErrorCode::Conflict => "The operation conflicts with an existing record or writer.",
            ErrorCode::IncompatibleVersion => "The contract or database version is incompatible.",
            ErrorCode::MigrationFailed => "Migration failed; retain the source and recovery copy.",
            ErrorCode::Cancelled => "The operation was cancelled.",
            ErrorCode::TimedOut => "The operation exceeded its time limit.",
            ErrorCode::OutputLimit => "The operation exceeded its output limit.",
            ErrorCode::RemoteRejected => {
                "The server rejected this change. Refresh and inspect the target before retrying."
            }
            ErrorCode::OutcomeUnknown => {
                "The server may have applied this change. Open a new session and inspect the target before retrying."
            }
        };
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for AtlasError {}

macro_rules! identity {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, TS)]
        #[serde(transparent)]
        pub struct $name(String);
        impl $name {
            pub fn parse(value: &str) -> Result<Self, AtlasError> {
                let valid = validation::text(value, 64, false)?;
                if valid != value {
                    return Err(AtlasError::new(ErrorCode::InvalidInput));
                }
                Ok(Self(valid))
            }
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().simple().to_string())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}
identity!(ServerId);
identity!(EdgeId);
identity!(SessionId);
identity!(OperationId);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct FoundationStatus {
    pub contract_version: u32,
    pub version: String,
    pub platform: String,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    SavedInventory,
    NetworkCheck,
    AuthenticatedSsh,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Observation {
    pub source: ObservationSource,
    pub observed_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Saved,
    Stale,
    Unknown,
}

impl Freshness {
    pub fn for_saved(observed_at: &str, now: DateTime<Utc>) -> Result<Self, AtlasError> {
        let at = validation::observation_time(observed_at, now)?;
        Ok(
            if now.signed_duration_since(at).num_seconds() > STALE_AFTER_SECONDS {
                Self::Stale
            } else {
                Self::Saved
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Reachability {
    Unknown,
    Reachable,
    Unreachable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct HostReachability {
    pub state: Reachability,
    pub last_checked_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
}

impl Default for HostReachability {
    fn default() -> Self {
        Self {
            state: Reachability::Unknown,
            last_checked_at: None,
            last_success_at: None,
            last_failure_at: None,
        }
    }
}

/// Persistable allowlisted inventory, not an authenticated inspection result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BaselineInventory {
    pub hostname: String,
    pub collected_at: String,
    /// SSH alias authenticated by Atlas for this complete refresh. Older and
    /// imported observations remain readable but cannot grant live workloads.
    #[serde(default)]
    pub source_host: Option<String>,
    /// Digest of the resolved SSH profile, authenticated remote key and account
    /// used for this complete refresh. Imports deliberately cannot supply it.
    #[serde(default)]
    pub source_binding: Option<String>,
    #[ts(type = "number")]
    pub cpus: u64,
    #[ts(type = "number")]
    pub memory_total_kib: u64,
    pub services: Vec<ServiceSummary>,
    pub containers: Vec<ContainerSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum InventoryRefreshStatus {
    Updated,
    Unchanged,
    Partial,
}

/// Ephemeral result of an explicit authenticated inventory refresh.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InventoryRefreshResult {
    pub contract_version: u32,
    pub server_id: ServerId,
    pub status: InventoryRefreshStatus,
    /// Timestamp reported by the remote collection command.
    pub source_collected_at: Option<String>,
    /// Local time Atlas received and handled the refresh result.
    pub refreshed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ServiceSummary {
    pub scope: String,
    pub name: String,
    pub state: String,
    pub substate: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContainerSummary {
    pub scope: String,
    pub engine: String,
    pub name: String,
    pub image: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MapServer {
    pub id: ServerId,
    pub name: String,
    pub host: String,
    pub x: f64,
    pub y: f64,
    pub created_at: String,
    pub inventory: Option<BaselineInventory>,
    pub freshness: Freshness,
    pub reachability: HostReachability,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Relationship {
    pub id: EdgeId,
    pub source: ServerId,
    pub target: ServerId,
    pub label: String,
    /// The plugin relationship kind chosen when it was documented, if any.
    #[serde(default)]
    pub kind: Option<RelationshipKindRef>,
}

/// The relationship kind a map relationship was documented with. The title
/// is kept with the relationship, so it still reads the same if the plugin
/// is later disabled or removed; only the line style then falls back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RelationshipKindRef {
    pub plugin_id: String,
    pub kind_id: String,
    pub title: String,
}

/// Which plugin relationship kind to document a relationship with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RelationshipKindChoice {
    pub plugin_id: String,
    pub contribution_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Activity {
    pub id: String,
    pub at: String,
    pub message: String,
    pub server_id: Option<ServerId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MapState {
    pub contract_version: u32,
    pub servers: Vec<MapServer>,
    pub edges: Vec<Relationship>,
    pub events: Vec<Activity>,
    pub served_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AddServer {
    pub name: String,
    pub host: String,
}

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateServer {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub host: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub x: Option<f64>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub y: Option<f64>,
}

impl UpdateServer {
    pub fn validate(&self) -> Result<(), AtlasError> {
        if self.name.is_none() && self.host.is_none() && self.x.is_none() && self.y.is_none() {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        if let Some(name) = &self.name {
            validation::text(name, 80, false)?;
        }
        if let Some(host) = &self.host {
            validation::hostname(host)?;
        }
        if let Some(x) = self.x {
            validation::coordinate(x)?;
        }
        if let Some(y) = self.y {
            validation::coordinate(y)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AddRelationship {
    pub source: ServerId,
    pub target: ServerId,
    pub label: String,
    /// A relationship kind from an enabled plugin; absent for a plain one.
    #[serde(default)]
    #[ts(optional)]
    pub kind: Option<RelationshipKindChoice>,
}

/// Ephemeral authenticated data: intentionally not part of BaselineInventory.
/// Actual session authorisation/expiry enforcement belongs to F05/F06.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProtectedText {
    pub session_id: SessionId,
    pub operation_id: OperationId,
    pub expires_at: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LiveMetrics {
    pub cpu_percent: Option<f64>,
    #[ts(type = "number | null")]
    pub memory_used_kib: Option<u64>,
    #[ts(type = "number | null")]
    pub disk_used_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OperationPhase {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OperationProgress {
    pub operation_id: OperationId,
    pub session_id: SessionId,
    pub phase: OperationPhase,
    #[ts(type = "number")]
    pub bytes_completed: u64,
    #[ts(type = "number | null")]
    pub bytes_total: Option<u64>,
}

impl OperationProgress {
    pub fn validate(&self) -> Result<(), AtlasError> {
        if self.bytes_completed > MAX_SAFE_INTEGER
            || self
                .bytes_total
                .is_some_and(|n| n > MAX_SAFE_INTEGER || n < self.bytes_completed)
        {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        Ok(())
    }
}

/// Bindings are generated explicitly; ordinary tests/checks never rewrite them.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WorkspaceStatus {
    pub contract_version: u32,
    pub storage_ready: bool,
    pub report_import_available: bool,
    pub coordinator_available: bool,
    pub pending_operations: u32,
}

pub fn typescript_contracts() -> String {
    let cfg = Config::default();
    let mut output = format!(
        "// Generated by atlas-core; run mise run contracts:generate.\nexport const CONTRACT_VERSION = {CONTRACT_VERSION} as const;\n"
    );
    macro_rules! append { ($($ty:ty),+ $(,)?) => { $(output.push_str("export "); output.push_str(&<$ty>::decl(&cfg)); output.push('\n');)+ }; }
    append!(
        ErrorCode,
        AtlasError,
        ServerId,
        EdgeId,
        SessionId,
        OperationId,
        FoundationStatus,
        WorkspaceStatus,
        ObservationSource,
        Observation,
        Freshness,
        Reachability,
        Health,
        HostReachability,
        ProbeKind,
        HealthProbeConfig,
        ServiceHealth,
        HealthSnapshot,
        BaselineInventory,
        InventoryRefreshStatus,
        InventoryRefreshResult,
        ServiceSummary,
        ContainerSummary,
        MapServer,
        RelationshipKindRef,
        Relationship,
        Activity,
        MapState,
        AddServer,
        UpdateServer,
        RelationshipKindChoice,
        AddRelationship,
        ProtectedText,
        LiveMetrics,
        OperationPhase,
        OperationProgress
    );
    output.push_str(&crate::operations::typescript_contracts(&cfg));
    output.push_str(&crate::monitoring::typescript_contracts(&cfg));
    output.push_str(&crate::ssh::typescript_contracts(&cfg));
    output.push_str(&crate::tunnel::typescript_contracts(&cfg));
    output.push_str(&crate::files::typescript_contracts());
    output.push_str(&crate::workloads::typescript_contracts());
    output.push_str(&crate::extensions::typescript_contracts(&cfg));
    output.push_str(&crate::pack::typescript_contracts(&cfg));
    output.push_str(&crate::plugin_install::typescript_contracts(&cfg));
    output.push_str(&crate::plugin_host::typescript_contracts(&cfg));
    let mut output = output
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    output.push('\n');
    output
}
