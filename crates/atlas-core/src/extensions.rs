//! Extension points (S12/F18). Every workspace tab, server-card badge,
//! settings section, collector, health check and operation is a registered
//! contribution with a stable ID, an owner and the capabilities it uses.
//!
//! The registry describes and authorises; it grants nothing by itself. SSH
//! authentication, session scope and elevation rules are enforced where they
//! always were. Every contribution is owned by an extension. Today those are
//! the core (`builtin`) and one optional built-in extension, remote
//! applications (`builtin.remote-apps`, T038), which can be switched off.
//!
//! What `authorize` proves today: the operation, collector or check about to
//! run is registered and declared the capability it needs, so removing a
//! contribution turns it off. It does not yet know which tab or plugin asked;
//! the capabilities declared by interface contributions are descriptive until
//! F20 adds caller identity and owner grants.
use crate::health::ProbeKind;
use crate::operations::Inspection;
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use ts_rs::{Config, TS};

/// Owner of every contribution compiled into the application.
pub const BUILTIN_OWNER: &str = "builtin";
/// The remote-applications feature (F17), delivered as a built-in extension.
pub const REMOTE_APPS_OWNER: &str = "builtin.remote-apps";
/// Comma-separated extension IDs to leave out at start-up.
pub const DISABLED_EXTENSIONS_ENV: &str = "ATLAS_DISABLED_EXTENSIONS";
pub const MAX_CONTRIBUTIONS: usize = 512;

/// Where a contribution attaches.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionPoint {
    /// A tab in a server's workspace.
    WorkspaceTab,
    /// A badge on a server's card on the map.
    ServerBadge,
    /// A section of a server's Settings tab (settings about one server).
    SettingsSection,
    /// A section of the application-wide Settings screen.
    AppSettingsSection,
    /// An entry in the main menu that opens one application view.
    MenuItem,
    /// Collects approved inventory from a server.
    Collector,
    /// An unauthenticated reachability check.
    HealthCheck,
    /// A brokered operation on a server.
    Operation,
    /// A named application in a server's Apps list, opened through the
    /// same loopback tunnel as any other (plugin API v2).
    RemoteApp,
    /// A named kind of relationship for the map: a label and a line style.
    /// It documents intent and applies nothing (plugin API v2).
    RelationshipKind,
    /// A plugin job: a sandboxed module that reads and may ask for the
    /// workload controls it declares (plugin API v2, D061).
    Job,
}

/// What a contribution may use. A contribution must declare every capability
/// it relies on; `ExtensionRegistry::authorize` refuses anything undeclared.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Read the saved map, approved inventory and activity on this PC.
    ReadSavedState,
    /// Change map entries on this PC. Never changes a server.
    EditMap,
    /// Unauthenticated TCP/HTTP reachability checks.
    NetworkProbe,
    /// Verify and use the owner's existing OpenSSH access.
    SshSession,
    /// Run a fixed, approved read-only command in an authenticated session.
    RunApprovedCommand,
    /// Read remote files and metadata over SFTP.
    ReadFiles,
    /// Create, rename, move or delete remote files after confirmation.
    WriteFiles,
    /// Upload and download between this PC and a server.
    TransferFiles,
    /// The owner's interactive terminal in an authenticated session.
    InteractiveTerminal,
    /// Ask for sudo for one named action.
    Elevate,
    /// Start, stop or restart a discovered workload after confirmation.
    ControlWorkloads,
    /// Open a loopback-only SSH forward to a server's own localhost.
    LoopbackTunnel,
}

impl ExtensionPoint {
    /// Capabilities a contribution at this point may declare.
    pub fn permitted(self) -> &'static [Capability] {
        use Capability::*;
        match self {
            // Interface contributions may surface any capability their
            // underlying operations use; the operation is what is enforced.
            Self::WorkspaceTab | Self::SettingsSection => &[
                ReadSavedState,
                EditMap,
                NetworkProbe,
                SshSession,
                RunApprovedCommand,
                ReadFiles,
                WriteFiles,
                TransferFiles,
                InteractiveTerminal,
                Elevate,
                ControlWorkloads,
                LoopbackTunnel,
            ],
            Self::ServerBadge | Self::MenuItem | Self::RelationshipKind => &[ReadSavedState],
            Self::RemoteApp => &[SshSession, LoopbackTunnel],
            // Application-wide settings change what InfraWerf stores on this
            // PC; they never contact a server.
            Self::AppSettingsSection => &[ReadSavedState, EditMap],
            Self::Collector => &[SshSession, RunApprovedCommand],
            Self::Job => &[
                ReadSavedState,
                SshSession,
                RunApprovedCommand,
                ControlWorkloads,
                Elevate,
            ],
            Self::HealthCheck => &[NetworkProbe, SshSession, RunApprovedCommand],
            Self::Operation => &[
                SshSession,
                RunApprovedCommand,
                ReadFiles,
                WriteFiles,
                TransferFiles,
                InteractiveTerminal,
                Elevate,
                ControlWorkloads,
                LoopbackTunnel,
            ],
        }
    }
}

/// One registered contribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    /// Stable dotted identifier, for example `builtin.tab.files`.
    pub id: String,
    pub point: ExtensionPoint,
    /// Display name; for tabs this is the visible label.
    pub title: String,
    pub owner: String,
    /// Position among contributions at the same point.
    pub order: u16,
    pub capabilities: Vec<Capability>,
}

/// One extension: the unit that owns contributions and is enabled or disabled
/// as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExtensionInfo {
    /// The owner ID its contributions carry.
    pub id: String,
    pub title: String,
    pub enabled: bool,
    /// Whether it can be disabled. The core cannot.
    pub optional: bool,
}

/// The registry as shown to the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExtensionCatalogue {
    pub contract_version: u32,
    /// Every known extension, including disabled ones.
    pub extensions: Vec<ExtensionInfo>,
    /// Contributions of enabled extensions only.
    pub contributions: Vec<Contribution>,
}

fn invalid() -> AtlasError {
    AtlasError::new(ErrorCode::InvalidInput)
}

/// Lower-case dotted identifier with at least two segments.
fn valid_id(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    id.len() <= 96
        && segments.len() >= 2
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && !segment.starts_with('-')
                && !segment.ends_with('-')
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

#[derive(Debug, Clone, Default)]
pub struct ExtensionRegistry {
    extensions: Vec<ExtensionInfo>,
    entries: Vec<Contribution>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a contribution. Rejects a malformed or duplicate ID, an empty or
    /// repeated capability list and any capability its point does not permit.
    pub fn register(&mut self, contribution: Contribution) -> Result<(), AtlasError> {
        if !valid_id(&contribution.id) || !valid_id(&format!("owner.{}", contribution.owner)) {
            return Err(invalid());
        }
        if text(&contribution.title, 40, false)? != contribution.title {
            return Err(invalid());
        }
        let declared: BTreeSet<Capability> = contribution.capabilities.iter().copied().collect();
        if declared.is_empty()
            || declared.len() != contribution.capabilities.len()
            || declared
                .iter()
                .any(|capability| !contribution.point.permitted().contains(capability))
        {
            return Err(invalid());
        }
        if self.entries.len() >= MAX_CONTRIBUTIONS
            || self.entries.iter().any(|e| e.id == contribution.id)
        {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        self.entries.push(contribution);
        Ok(())
    }

    /// Records an extension. A disabled one is listed but contributes nothing.
    pub fn declare(&mut self, extension: ExtensionInfo) -> Result<(), AtlasError> {
        if !valid_id(&format!("owner.{}", extension.id))
            || text(&extension.title, 40, false)? != extension.title
            || (!extension.optional && !extension.enabled)
        {
            return Err(invalid());
        }
        if self.extensions.iter().any(|e| e.id == extension.id) {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        self.extensions.push(extension);
        Ok(())
    }

    /// Confirms an extension is present and enabled; its commands call this
    /// first, so a disabled extension's commands do nothing.
    pub fn require(&self, extension: &str) -> Result<(), AtlasError> {
        if self
            .extensions
            .iter()
            .any(|e| e.id == extension && e.enabled)
        {
            Ok(())
        } else {
            Err(AtlasError::new(ErrorCode::NotFound))
        }
    }

    pub fn get(&self, id: &str) -> Option<&Contribution> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Contributions at one point, in display order.
    pub fn at(&self, point: ExtensionPoint) -> Vec<&Contribution> {
        let mut found: Vec<&Contribution> =
            self.entries.iter().filter(|e| e.point == point).collect();
        found.sort_by(|a, b| (a.order, &a.id).cmp(&(b.order, &b.id)));
        found
    }

    /// Confirms a registered contribution declared the capability it is about
    /// to use. Unknown contributions and undeclared capabilities are refused.
    pub fn authorize(&self, id: &str, capability: Capability) -> Result<(), AtlasError> {
        let entry = self
            .get(id)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        if !entry.capabilities.contains(&capability) {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(())
    }

    pub fn catalogue(&self) -> ExtensionCatalogue {
        let mut contributions = self.entries.clone();
        contributions.sort_by(|a, b| (a.point, a.order, &a.id).cmp(&(b.point, b.order, &b.id)));
        ExtensionCatalogue {
            contract_version: CONTRACT_VERSION,
            extensions: self.extensions.clone(),
            contributions,
        }
    }
}

pub const COLLECTOR_BASELINE_INVENTORY: &str = "builtin.collector.baseline-inventory";
/// Operations that are not an `Inspection`: the two session bootstraps and the
/// remote-application tunnel.
pub const OPERATION_CONNECTION_CHECK: &str = "builtin.operation.connection-check";
pub const OPERATION_SFTP_OPEN: &str = "builtin.operation.sftp-open";
pub const OPERATION_LOOPBACK_TUNNEL: &str = "builtin.operation.loopback-tunnel";
/// Runs one command a reviewed and enabled plugin pack declared (F21/T043).
pub const OPERATION_PACK_COMMAND: &str = "builtin.operation.pack-command";

/// Registry entry behind a health probe kind.
pub fn health_check_id(kind: ProbeKind) -> &'static str {
    match kind {
        ProbeKind::Tcp => "builtin.health.tcp",
        ProbeKind::Http => "builtin.health.http",
    }
}

/// Registry entry and capability behind each brokered operation.
pub fn operation_entry(inspection: &Inspection) -> (&'static str, Capability) {
    use Capability::*;
    match inspection {
        Inspection::SystemSummary {} => ("builtin.operation.system-summary", RunApprovedCommand),
        Inspection::ApplicationDiscovery {} => (
            "builtin.operation.application-discovery",
            RunApprovedCommand,
        ),
        Inspection::PackCommand {} => (OPERATION_PACK_COMMAND, RunApprovedCommand),
        Inspection::FileMetadata { .. } => ("builtin.operation.file-metadata", ReadFiles),
        Inspection::ServiceStatus { .. } => {
            ("builtin.operation.service-status", RunApprovedCommand)
        }
        Inspection::WorkloadControl { .. } => {
            ("builtin.operation.workload-control", ControlWorkloads)
        }
        Inspection::SftpRead { .. } => ("builtin.operation.sftp-read", ReadFiles),
        Inspection::SftpMutation { .. } => ("builtin.operation.sftp-mutation", WriteFiles),
        Inspection::SftpTransfer { .. } => ("builtin.operation.sftp-transfer", TransferFiles),
        Inspection::ElevationCheck {} => ("builtin.operation.elevation-check", Elevate),
        Inspection::ProtectedPlacement {} => ("builtin.operation.protected-placement", Elevate),
        Inspection::TerminalOpen {} => ("builtin.operation.terminal", InteractiveTerminal),
    }
}

/// Which built-in extension owns a built-in contribution.
fn builtin_owner(id: &str) -> &'static str {
    match id {
        "builtin.tab.apps"
        | "builtin.operation.application-discovery"
        | OPERATION_LOOPBACK_TUNNEL => REMOTE_APPS_OWNER,
        _ => BUILTIN_OWNER,
    }
}

/// Extension IDs from a comma-separated list, as given in
/// `ATLAS_DISABLED_EXTENSIONS`. Unknown IDs and the core are ignored by
/// `builtin_with`.
pub fn parse_disabled(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Everything compiled into InfraWerf with every extension enabled.
pub fn builtin() -> ExtensionRegistry {
    builtin_with(&[])
}

/// Everything compiled into InfraWerf, registered through the same contract a
/// plugin will use, leaving out the contributions of each disabled optional
/// extension. The core cannot be disabled.
pub fn builtin_with(disabled: &[String]) -> ExtensionRegistry {
    use Capability::*;
    use ExtensionPoint::*;
    let mut registry = ExtensionRegistry::new();
    for (id, title, optional) in [
        (BUILTIN_OWNER, "InfraWerf core", false),
        (REMOTE_APPS_OWNER, "Remote applications", true),
    ] {
        registry
            .declare(ExtensionInfo {
                id: id.into(),
                title: title.into(),
                enabled: !optional || !disabled.iter().any(|d| d == id),
                optional,
            })
            .expect("built-in extensions are valid and unique");
    }
    let entries: &[(&str, ExtensionPoint, &str, u16, &[Capability])] = &[
        // Workspace tabs. Titles are the visible labels and must stay stable.
        (
            "builtin.tab.services",
            WorkspaceTab,
            "Services",
            10,
            &[
                ReadSavedState,
                SshSession,
                RunApprovedCommand,
                ControlWorkloads,
                Elevate,
            ],
        ),
        (
            "builtin.tab.health",
            WorkspaceTab,
            "Health",
            20,
            &[ReadSavedState, NetworkProbe],
        ),
        (
            "builtin.tab.files",
            WorkspaceTab,
            "Files",
            30,
            &[SshSession, ReadFiles, WriteFiles, TransferFiles, Elevate],
        ),
        (
            "builtin.tab.terminal",
            WorkspaceTab,
            "Terminal",
            40,
            &[SshSession, InteractiveTerminal],
        ),
        (
            "builtin.tab.apps",
            WorkspaceTab,
            "Apps",
            50,
            &[SshSession, RunApprovedCommand, LoopbackTunnel],
        ),
        (
            "builtin.tab.plugin-data",
            WorkspaceTab,
            "Plugin data",
            55,
            &[ReadSavedState, SshSession, RunApprovedCommand],
        ),
        (
            "builtin.tab.system",
            WorkspaceTab,
            "System",
            60,
            &[ReadSavedState, SshSession, RunApprovedCommand],
        ),
        (
            "builtin.tab.settings",
            WorkspaceTab,
            "Settings",
            70,
            &[ReadSavedState, EditMap, SshSession, Elevate],
        ),
        (
            "builtin.tab.transfers",
            WorkspaceTab,
            "Transfers",
            80,
            &[ReadSavedState, TransferFiles],
        ),
        // Server-card badges.
        (
            "builtin.badge.observation",
            ServerBadge,
            "Inventory observation",
            10,
            &[ReadSavedState],
        ),
        (
            "builtin.badge.live-status",
            ServerBadge,
            "Live status",
            20,
            &[ReadSavedState],
        ),
        (
            "builtin.badge.workloads",
            ServerBadge,
            "Workload preview",
            30,
            &[ReadSavedState],
        ),
        (
            "builtin.badge.plugins",
            ServerBadge,
            "Plugin badges",
            40,
            &[ReadSavedState],
        ),
        // Sections of the Settings tab.
        (
            "builtin.settings.map-details",
            SettingsSection,
            "Map details",
            10,
            &[ReadSavedState, EditMap],
        ),
        (
            "builtin.settings.ssh-connection",
            SettingsSection,
            "SSH connection",
            20,
            &[SshSession],
        ),
        (
            "builtin.settings.elevation",
            SettingsSection,
            "Elevation check",
            30,
            &[SshSession, Elevate],
        ),
        (
            "builtin.settings.plugins",
            SettingsSection,
            "Plugins on this server",
            40,
            &[ReadSavedState],
        ),
        // Sections of the application Settings screen.
        (
            "builtin.app-settings.history-retention",
            AppSettingsSection,
            "History retention",
            10,
            &[ReadSavedState, EditMap],
        ),
        (
            "builtin.app-settings.ssh-discovery",
            AppSettingsSection,
            "SSH alias discovery",
            20,
            &[ReadSavedState, EditMap],
        ),
        (
            "builtin.app-settings.menu-layout",
            AppSettingsSection,
            "Menu layout",
            30,
            &[ReadSavedState],
        ),
        (
            "builtin.app-settings.pin",
            AppSettingsSection,
            "PIN",
            35,
            &[ReadSavedState],
        ),
        (
            "builtin.app-settings.plugins",
            AppSettingsSection,
            "Plugins",
            40,
            &[ReadSavedState, EditMap],
        ),
        (
            "builtin.app-settings.plugin-activity",
            AppSettingsSection,
            "Plugin activity",
            50,
            &[ReadSavedState],
        ),
        // Main menu items. The interface places each in a category (T053).
        (
            "builtin.menu.topology",
            MenuItem,
            "Topology",
            10,
            &[ReadSavedState],
        ),
        (
            "builtin.menu.inventory",
            MenuItem,
            "Inventory",
            20,
            &[ReadSavedState],
        ),
        (
            "builtin.menu.transfers",
            MenuItem,
            "Transfers",
            30,
            &[ReadSavedState],
        ),
        (
            "builtin.menu.activity",
            MenuItem,
            "Activity",
            40,
            &[ReadSavedState],
        ),
        (
            "builtin.menu.servers",
            MenuItem,
            "Servers & monitoring",
            50,
            &[ReadSavedState],
        ),
        (
            "builtin.menu.settings",
            MenuItem,
            "Settings",
            60,
            &[ReadSavedState],
        ),
        // Collectors and health checks.
        (
            COLLECTOR_BASELINE_INVENTORY,
            Collector,
            "Approved baseline inventory",
            10,
            &[SshSession, RunApprovedCommand],
        ),
        (
            "builtin.health.tcp",
            HealthCheck,
            "TCP reachability",
            10,
            &[NetworkProbe],
        ),
        (
            "builtin.health.http",
            HealthCheck,
            "HTTP service check",
            20,
            &[NetworkProbe],
        ),
        // Brokered operations: the two bootstraps, then one per `Inspection`.
        (
            OPERATION_CONNECTION_CHECK,
            Operation,
            "SSH connection check",
            1,
            &[SshSession],
        ),
        (
            OPERATION_SFTP_OPEN,
            Operation,
            "Open file session",
            2,
            &[SshSession],
        ),
        (
            "builtin.operation.system-summary",
            Operation,
            "System summary",
            10,
            &[RunApprovedCommand],
        ),
        (
            "builtin.operation.application-discovery",
            Operation,
            "Application discovery",
            20,
            &[RunApprovedCommand],
        ),
        (
            OPERATION_PACK_COMMAND,
            Operation,
            "Plugin pack command",
            25,
            &[RunApprovedCommand],
        ),
        (
            "builtin.operation.file-metadata",
            Operation,
            "File metadata",
            30,
            &[ReadFiles],
        ),
        (
            "builtin.operation.service-status",
            Operation,
            "Workload status and logs",
            40,
            &[RunApprovedCommand],
        ),
        (
            "builtin.operation.workload-control",
            Operation,
            "Workload start, stop, restart",
            50,
            &[ControlWorkloads],
        ),
        (
            "builtin.operation.sftp-read",
            Operation,
            "Read files",
            60,
            &[ReadFiles],
        ),
        (
            "builtin.operation.sftp-mutation",
            Operation,
            "Change files",
            70,
            &[WriteFiles],
        ),
        (
            "builtin.operation.sftp-transfer",
            Operation,
            "Transfer files",
            80,
            &[TransferFiles],
        ),
        (
            "builtin.operation.elevation-check",
            Operation,
            "Elevation check",
            90,
            &[Elevate],
        ),
        (
            "builtin.operation.protected-placement",
            Operation,
            "Protected file placement",
            100,
            &[Elevate],
        ),
        (
            "builtin.operation.terminal",
            Operation,
            "Interactive terminal",
            110,
            &[InteractiveTerminal],
        ),
        (
            OPERATION_LOOPBACK_TUNNEL,
            Operation,
            "Remote application tunnel",
            120,
            &[LoopbackTunnel],
        ),
    ];
    for (id, point, title, order, capabilities) in entries {
        let owner = builtin_owner(id);
        if registry.require(owner).is_err() {
            continue;
        }
        registry
            .register(Contribution {
                id: (*id).into(),
                point: *point,
                title: (*title).into(),
                owner: owner.into(),
                order: *order,
                capabilities: capabilities.to_vec(),
            })
            .expect("built-in contributions are valid and unique");
    }
    registry
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        ExtensionPoint::decl(config),
        Capability::decl(config),
        ExtensionInfo::decl(config),
        Contribution::decl(config),
        ExtensionCatalogue::decl(config),
    ]
    .into_iter()
    .map(|d| format!("export {d}\n"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, point: ExtensionPoint, capabilities: &[Capability]) -> Contribution {
        Contribution {
            id: id.into(),
            point,
            title: "Example".into(),
            owner: BUILTIN_OWNER.into(),
            order: 1,
            capabilities: capabilities.to_vec(),
        }
    }

    #[test]
    fn builtin_registry_covers_every_extension_point() {
        let registry = builtin();
        for point in [
            ExtensionPoint::WorkspaceTab,
            ExtensionPoint::ServerBadge,
            ExtensionPoint::SettingsSection,
            ExtensionPoint::AppSettingsSection,
            ExtensionPoint::MenuItem,
            ExtensionPoint::Collector,
            ExtensionPoint::HealthCheck,
            ExtensionPoint::Operation,
        ] {
            assert!(!registry.at(point).is_empty(), "{point:?} has no entries");
        }
        let app_settings: Vec<&str> = registry
            .at(ExtensionPoint::AppSettingsSection)
            .iter()
            .map(|c| c.title.as_str())
            .collect();
        assert_eq!(
            app_settings,
            [
                "History retention",
                "SSH alias discovery",
                "Menu layout",
                "PIN",
                "Plugins",
                "Plugin activity"
            ]
        );
        let menu: Vec<&str> = registry
            .at(ExtensionPoint::MenuItem)
            .iter()
            .map(|c| c.title.as_str())
            .collect();
        assert_eq!(
            menu,
            [
                "Topology",
                "Inventory",
                "Transfers",
                "Activity",
                "Servers & monitoring",
                "Settings"
            ]
        );
        let tabs: Vec<&str> = registry
            .at(ExtensionPoint::WorkspaceTab)
            .iter()
            .map(|c| c.title.as_str())
            .collect();
        assert_eq!(
            tabs,
            [
                "Services",
                "Health",
                "Files",
                "Terminal",
                "Apps",
                "Plugin data",
                "System",
                "Settings",
                "Transfers"
            ]
        );
        assert!(
            registry
                .catalogue()
                .contributions
                .iter()
                .all(|c| registry.require(&c.owner).is_ok() && !c.capabilities.is_empty())
        );
    }

    #[test]
    fn disabling_remote_applications_removes_only_its_contributions() {
        let full = builtin();
        let ids = |registry: &ExtensionRegistry| -> BTreeSet<String> {
            registry
                .catalogue()
                .contributions
                .into_iter()
                .map(|c| c.id)
                .collect()
        };
        let owned: BTreeSet<String> = full
            .catalogue()
            .contributions
            .into_iter()
            .filter(|c| c.owner == REMOTE_APPS_OWNER)
            .map(|c| c.id)
            .collect();
        assert_eq!(
            owned.iter().map(String::as_str).collect::<Vec<_>>(),
            [
                "builtin.operation.application-discovery",
                OPERATION_LOOPBACK_TUNNEL,
                "builtin.tab.apps"
            ]
        );
        full.require(REMOTE_APPS_OWNER).unwrap();

        // The core is not optional; an unknown ID changes nothing.
        let disabled = builtin_with(&parse_disabled(
            " builtin.remote-apps, builtin ,unknown.extension,",
        ));
        let remaining = ids(&disabled);
        assert_eq!(
            ids(&full)
                .difference(&remaining)
                .cloned()
                .collect::<BTreeSet<_>>(),
            owned
        );
        assert!(remaining.is_subset(&ids(&full)));
        disabled.require(BUILTIN_OWNER).unwrap();
        assert_eq!(
            disabled.require(REMOTE_APPS_OWNER).unwrap_err().code,
            ErrorCode::NotFound
        );
        assert_eq!(
            disabled
                .authorize(OPERATION_LOOPBACK_TUNNEL, Capability::LoopbackTunnel)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        let (discovery, capability) = operation_entry(&Inspection::ApplicationDiscovery {});
        assert_eq!(
            disabled.authorize(discovery, capability).unwrap_err().code,
            ErrorCode::NotFound
        );
        // It is still listed, as disabled, so the interface can say so.
        let listed = disabled.catalogue().extensions;
        assert_eq!(
            listed
                .iter()
                .map(|e| (e.id.as_str(), e.enabled, e.optional))
                .collect::<Vec<_>>(),
            [
                (BUILTIN_OWNER, true, false),
                (REMOTE_APPS_OWNER, false, true)
            ]
        );
        // The core cannot be declared disabled.
        let mut registry = ExtensionRegistry::new();
        assert_eq!(
            registry
                .declare(ExtensionInfo {
                    id: "example".into(),
                    title: "Example".into(),
                    enabled: false,
                    optional: false,
                })
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn every_health_check_and_operation_is_registered_with_its_capability() {
        let registry = builtin();
        for kind in [ProbeKind::Tcp, ProbeKind::Http] {
            registry
                .authorize(health_check_id(kind), Capability::NetworkProbe)
                .unwrap();
        }
        registry
            .authorize(COLLECTOR_BASELINE_INVENTORY, Capability::RunApprovedCommand)
            .unwrap();
        for (id, capability) in [
            (OPERATION_CONNECTION_CHECK, Capability::SshSession),
            (OPERATION_SFTP_OPEN, Capability::SshSession),
            (OPERATION_LOOPBACK_TUNNEL, Capability::LoopbackTunnel),
        ] {
            assert_eq!(registry.get(id).unwrap().point, ExtensionPoint::Operation);
            registry.authorize(id, capability).unwrap();
        }
        let operations = [
            Inspection::SystemSummary {},
            Inspection::ApplicationDiscovery {},
            Inspection::FileMetadata { path: "/".into() },
            Inspection::ServiceStatus {
                workload_id: "w".into(),
            },
            Inspection::WorkloadControl {
                workload_id: "w".into(),
            },
            Inspection::SftpRead {
                resource_id: "r".into(),
            },
            Inspection::SftpMutation {
                resource_id: "r".into(),
            },
            Inspection::SftpTransfer {
                resource_id: "r".into(),
            },
            Inspection::ElevationCheck {},
            Inspection::ProtectedPlacement {},
            Inspection::TerminalOpen {},
        ];
        for operation in &operations {
            let (id, capability) = operation_entry(operation);
            assert_eq!(registry.get(id).unwrap().point, ExtensionPoint::Operation);
            registry.authorize(id, capability).unwrap();
        }
    }

    #[test]
    fn duplicate_and_malformed_ids_are_rejected() {
        let mut registry = ExtensionRegistry::new();
        let tab = entry(
            "example.tab.one",
            ExtensionPoint::WorkspaceTab,
            &[Capability::ReadSavedState],
        );
        registry.register(tab.clone()).unwrap();
        assert_eq!(
            registry.register(tab).unwrap_err().code,
            ErrorCode::Conflict
        );
        for id in [
            "",
            "single",
            "Upper.case",
            "has space.x",
            "a..b",
            "a.-b",
            "a.b-",
            "a/b.c",
        ] {
            let bad = entry(
                id,
                ExtensionPoint::WorkspaceTab,
                &[Capability::ReadSavedState],
            );
            assert_eq!(
                registry.register(bad).unwrap_err().code,
                ErrorCode::InvalidInput,
                "{id:?}"
            );
        }
    }

    #[test]
    fn capabilities_must_be_declared_and_permitted_for_the_point() {
        let mut registry = ExtensionRegistry::new();
        // None declared, repeated, and not permitted for a badge or a check.
        for (point, capabilities) in [
            (ExtensionPoint::WorkspaceTab, &[][..]),
            (
                ExtensionPoint::WorkspaceTab,
                &[Capability::ReadFiles, Capability::ReadFiles][..],
            ),
            (ExtensionPoint::ServerBadge, &[Capability::SshSession][..]),
            // Application settings never reach a server.
            (
                ExtensionPoint::AppSettingsSection,
                &[Capability::SshSession][..],
            ),
            // A menu item only opens a view.
            (ExtensionPoint::MenuItem, &[Capability::EditMap][..]),
            (ExtensionPoint::HealthCheck, &[Capability::Elevate][..]),
            (ExtensionPoint::Collector, &[Capability::WriteFiles][..]),
        ] {
            assert_eq!(
                registry
                    .register(entry("example.bad.entry", point, capabilities))
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidInput
            );
        }
        registry
            .register(entry(
                "example.operation.read",
                ExtensionPoint::Operation,
                &[Capability::ReadFiles],
            ))
            .unwrap();
        registry
            .authorize("example.operation.read", Capability::ReadFiles)
            .unwrap();
        // An undeclared capability and an unknown contribution are refused.
        assert_eq!(
            registry
                .authorize("example.operation.read", Capability::WriteFiles)
                .unwrap_err()
                .code,
            ErrorCode::Denied
        );
        assert_eq!(
            registry
                .authorize("example.operation.missing", Capability::ReadFiles)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
    }

    #[test]
    fn titles_are_bounded_and_trimmed() {
        let mut registry = ExtensionRegistry::new();
        for title in ["", "  padded  ", &"x".repeat(41), "line\nbreak"] {
            let mut bad = entry(
                "example.tab.title",
                ExtensionPoint::WorkspaceTab,
                &[Capability::ReadSavedState],
            );
            bad.title = title.into();
            assert_eq!(
                registry.register(bad).unwrap_err().code,
                ErrorCode::InvalidInput,
                "{title:?}"
            );
        }
    }
}
