//! Installed plugins, owner grants and the enforcement point (S12/F20/T041).
//!
//! A plugin is a folder in the plugins directory holding a valid manifest.
//! Finding one grants nothing. The owner enables a plugin by choosing a
//! trust level (nothing is preselected) and grants capabilities per server,
//! always from the plugin's own page. `authorize` is the one place a plugin
//! call is admitted or refused; it runs here in the core, never in the
//! interface. Every call it answers, allowed or refused, and every change
//! the owner makes is written to the plugin activity log (T042), which is
//! bounded like the rest of the history and holds only identifiers, names
//! of capabilities and outcomes: never output, arguments or credentials.
//! One switch turns every plugin off at once.
//!
//! No grant reaches keys, passwords or an unrestricted shell: the manifest
//! cannot request the interactive terminal, and elevation is refused until
//! the owner can name an exception for one action (T042).
use crate::extensions::{Capability, ExtensionPoint};
use crate::pack::{PackAppView, PackRelationshipKindView};
use crate::plugin::{MANIFEST_FILE, MAX_MANIFEST_BYTES, PluginManifest, validate_manifest};
use crate::store::sql_error;
use crate::*;
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use ts_rs::{Config, TS};

/// Overrides where plugins are looked for (used by tests).
pub const PLUGINS_DIR_ENV: &str = "ATLAS_PLUGINS_DIR";
pub const MAX_PLUGIN_FOLDERS: usize = 64;

/// Added without a schema version change: older releases ignore these tables.
pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS plugin_state (
  plugin_id TEXT PRIMARY KEY, trust TEXT NOT NULL, manifest_hash TEXT NOT NULL, enabled_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS plugin_grant (
  plugin_id TEXT NOT NULL, server_id TEXT NOT NULL, capability TEXT NOT NULL,
  PRIMARY KEY(plugin_id, server_id, capability));
CREATE TABLE IF NOT EXISTS plugin_activity (
  id INTEGER PRIMARY KEY, at TEXT NOT NULL, plugin_id TEXT NOT NULL, plugin_name TEXT NOT NULL,
  server_id TEXT, action TEXT NOT NULL, outcome TEXT NOT NULL, detail TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS plugin_switch (id INTEGER PRIMARY KEY CHECK(id=1), all_off INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS plugin_install (
 plugin_id TEXT PRIMARY KEY, folder TEXT NOT NULL, source_kind TEXT NOT NULL, source TEXT NOT NULL,
 content_hash TEXT NOT NULL, archive_hash TEXT, signer_key_id TEXT, installed_at TEXT NOT NULL,
 integrity TEXT NOT NULL, checked_at TEXT);
CREATE TABLE IF NOT EXISTS edge_kind (
 edge_id TEXT PRIMARY KEY REFERENCES edges(id) ON DELETE CASCADE,
 plugin_id TEXT NOT NULL, kind_id TEXT NOT NULL, title TEXT NOT NULL);
";
/// The most log entries returned to the interface at once.
pub const MAX_ACTIVITY_PAGE: u32 = 500;
/// Actions in the log that are not a capability.
pub const ACTION_ENABLE: &str = "enable";
pub const ACTION_DISABLE: &str = "disable";
pub const ACTION_SET_PERMISSIONS: &str = "set_permissions";
pub const ACTION_REVOKE_TRUSTED: &str = "revoke_trusted_automation";
pub const ACTION_ALL_OFF: &str = "all_plugins_off";
pub const ACTION_ALL_ON: &str = "all_plugins_on";
/// One run of a pack command; the log's detail is the contribution's ID.
pub const ACTION_RUN_COMMAND: &str = "run_command";
/// A plugin was installed; the detail is the kind of source.
pub const ACTION_INSTALL: &str = "install";
/// A plugin's files, permissions and settings were removed.
pub const ACTION_REMOVE: &str = "remove";
/// A plugin was replaced by a newer or older version; the detail says the
/// versions and whether it needs a new review.
pub const ACTION_UPDATE: &str = "update";
/// A plugin view was opened on a server; the detail is its contribution.
pub const ACTION_OPEN_VIEW: &str = "open_view";
/// A plugin view tried something that was blocked; the detail says what.
pub const ACTION_VIEW_BLOCKED: &str = "view_blocked";
/// A plugin view asked to run a command; the detail is the contribution.
pub const ACTION_VIEW_REQUEST: &str = "view_request";
/// A pack application was opened through a tunnel; the detail is its ID.
pub const ACTION_OPEN_APP: &str = "open_app";

/// How far an enabled plugin may act. Chosen by the owner each time a plugin
/// is enabled; there is no default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Read and observe only.
    ReadOnly,
    /// May change things, each change confirmed by the owner first.
    ConfirmChanges,
    /// May change things without asking each time.
    TrustedAutomation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Read,
    Change,
    Elevation,
}

pub fn capability_kind(capability: Capability) -> CapabilityKind {
    use Capability::*;
    match capability {
        ReadSavedState | NetworkProbe | SshSession | RunApprovedCommand | ReadFiles => {
            CapabilityKind::Read
        }
        EditMap | WriteFiles | TransferFiles | ControlWorkloads | LoopbackTunnel
        | InteractiveTerminal => CapabilityKind::Change,
        Elevate => CapabilityKind::Elevation,
    }
}

/// What a capability means, in the words the owner is shown.
pub fn capability_summary(capability: Capability) -> &'static str {
    use Capability::*;
    match capability {
        ReadSavedState => {
            "Read what InfraWerf has saved about this server on this PC: its map entry, approved inventory and activity."
        }
        EditMap => "Change this server's entry on your map. Never changes the server itself.",
        NetworkProbe => {
            "Check whether this server answers on the network, without signing in to it."
        }
        SshSession => {
            "Sign in to this server through InfraWerf with your existing SSH access. The plugin never receives your keys or a raw session."
        }
        RunApprovedCommand => "Run fixed, read-only commands on this server.",
        ReadFiles => "Read files and folders on this server.",
        WriteFiles => "Create, rename, move or delete files on this server.",
        TransferFiles => "Upload files to this server and download files from it.",
        InteractiveTerminal => "Use your interactive terminal. No plugin may have this.",
        Elevate => "Ask to use sudo on this server for one named action.",
        ControlWorkloads => "Start, stop or restart services and containers on this server.",
        LoopbackTunnel => {
            "Open a private connection from this PC to an application on this server."
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Disabled,
    Enabled,
    /// Its manifest is not the one the owner reviewed; it is treated as
    /// disabled until it is reviewed and enabled again.
    ChangedSinceReview,
    /// Its files no longer match the hash recorded when it was installed.
    /// Nothing it asks for is allowed until it is removed and installed
    /// again.
    FilesChanged,
}

/// Why a plugin call was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    UnknownPlugin,
    PluginDisabled,
    ChangedSinceReview,
    UnknownServer,
    /// The manifest does not request this capability.
    NotRequested,
    /// The owner has not granted it on this server.
    NotGranted,
    /// The trust level chosen for the plugin does not allow changes.
    TrustTooLow,
    /// Sudo needs a named exception, and none exists.
    NoElevationException,
    /// The owner has turned every plugin off.
    AllPluginsOff,
    /// The plugin's files changed after it was installed.
    FilesChanged,
}

/// How one logged action ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ActivityOutcome {
    Allowed,
    NeedsConfirmation,
    Refused,
    /// A change the owner made on a plugin's page, or a pack command that
    /// ran to the end.
    Done,
    /// A pack command stopped because its permission changed while it ran.
    Cancelled,
    /// A pack command that could not be run or read.
    Failed,
}

/// One line of the plugin activity log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginActivity {
    pub id: u32,
    pub at: String,
    /// Empty for an action on all plugins.
    pub plugin_id: String,
    pub plugin_name: String,
    pub server_id: Option<ServerId>,
    /// A capability name, or one of the owner actions such as `enable`.
    pub action: String,
    pub outcome: ActivityOutcome,
    /// The refusal, the trust level or the permissions set; never output.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginActivityLog {
    pub contract_version: u32,
    /// How many entries are kept: the history retention setting.
    pub kept: u32,
    /// Newest first.
    pub entries: Vec<PluginActivity>,
}

/// The answer to one plugin call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginDecision {
    pub contract_version: u32,
    pub allowed: bool,
    /// Allowed only after the owner confirms this one change.
    pub needs_confirmation: bool,
    pub refusal: Option<Refusal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RequestedCapability {
    pub capability: Capability,
    pub kind: CapabilityKind,
    pub summary: String,
    /// Titles of the contributions that use it.
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginContributionView {
    pub id: String,
    pub title: String,
    pub point: ExtensionPoint,
    pub capabilities: Vec<Capability>,
    /// The exact read-only command a pack contribution runs, if any.
    pub command: Option<crate::plugin::PackCommand>,
    pub workload_kind: Option<crate::plugin::PackWorkloadKind>,
    pub app: Option<crate::plugin::PackApp>,
    pub badge: Option<crate::plugin::PackBadge>,
    pub relationship: Option<crate::plugin::PackRelationship>,
    pub view: Option<crate::plugin::PluginViewSpec>,
}

/// A plugin view tab the owner's settings admit on one server right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginViewTab {
    pub plugin_id: String,
    pub plugin_name: String,
    pub contribution_id: String,
    pub title: String,
}

/// The plugin view tabs for one server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginViewsState {
    pub contract_version: u32,
    pub server_id: ServerId,
    pub views: Vec<PluginViewTab>,
}

/// Where an admitted view's files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginViewSource {
    pub tab: PluginViewTab,
    pub folder: std::path::PathBuf,
    pub entry: String,
}

/// A pack badge the owner's settings admit on one server right now, and
/// the contribution whose observation it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackBadgeBinding {
    pub server_id: ServerId,
    pub plugin_id: String,
    pub plugin_name: String,
    pub contribution_id: String,
    pub title: String,
    pub observation: String,
}

/// A pack command the owner's settings admit right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackRun {
    pub plugin_id: String,
    pub plugin_name: String,
    pub contribution: crate::plugin::ManifestContribution,
    pub command: crate::plugin::PackCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ServerGrant {
    pub server_id: ServerId,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: u32,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    /// The folder it was found in, inside the plugins directory.
    pub folder: String,
    pub status: PluginStatus,
    /// Set only while the plugin is enabled.
    pub trust: Option<TrustLevel>,
    pub capabilities: Vec<RequestedCapability>,
    pub contributions: Vec<PluginContributionView>,
    /// Servers with at least one granted capability.
    pub grants: Vec<ServerGrant>,
    /// How it was installed; absent for a folder placed by hand.
    pub install: Option<crate::plugin_install::InstallRecordView>,
}

/// A folder in the plugins directory that is not a usable plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InvalidPlugin {
    pub folder: String,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginsState {
    pub contract_version: u32,
    pub directory: String,
    /// The owner has turned every plugin off; nothing is admitted.
    pub all_off: bool,
    pub plugins: Vec<PluginView>,
    pub invalid: Vec<InvalidPlugin>,
}

/// A plugin found on disk with a valid manifest.
#[derive(Debug, Clone)]
pub struct Installed {
    pub manifest: PluginManifest,
    /// SHA-256 of the manifest file, to notice a change after review.
    pub hash: String,
    pub folder: String,
}

fn name<T: Serialize>(value: T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => text,
        _ => String::new(),
    }
}
fn parse<T: for<'a> Deserialize<'a>>(text: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(text.to_owned())).ok()
}
fn invalid() -> AtlasError {
    AtlasError::new(ErrorCode::InvalidInput)
}

/// Reads the plugins directory. Only manifests are read; nothing is run.
pub fn discover(directory: &Path) -> (Vec<Installed>, Vec<InvalidPlugin>) {
    let mut folders: Vec<std::path::PathBuf> = match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            // InfraWerf's own working folders are not plugins.
            .filter(|path| {
                !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            })
            .collect(),
        Err(_) => return (Vec::new(), Vec::new()),
    };
    folders.sort();
    let mut installed: Vec<Installed> = Vec::new();
    let mut rejected = Vec::new();
    for (index, path) in folders.iter().enumerate() {
        let folder: String = path
            .file_name()
            .map(|name| name.to_string_lossy().chars().take(80).collect())
            .unwrap_or_default();
        let mut reject = |problems: Vec<String>| {
            rejected.push(InvalidPlugin {
                folder: folder.clone(),
                problems,
            })
        };
        if index >= MAX_PLUGIN_FOLDERS {
            reject(vec![format!(
                "Only the first {MAX_PLUGIN_FOLDERS} plugin folders are read."
            )]);
            continue;
        }
        let mut bytes = Vec::new();
        let read = std::fs::File::open(path.join(MANIFEST_FILE)).and_then(|file| {
            file.take(MAX_MANIFEST_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        });
        if read.is_err() {
            reject(vec![format!("{MANIFEST_FILE} is missing or unreadable.")]);
            continue;
        }
        match validate_manifest(&bytes) {
            Err(issues) => reject(issues.iter().map(ToString::to_string).collect()),
            Ok(manifest) => {
                if let Some(first) = installed.iter().find(|p| p.manifest.id == manifest.id) {
                    reject(vec![format!(
                        "Its ID, {}, is already used by the plugin in \"{}\".",
                        manifest.id, first.folder
                    )]);
                    continue;
                }
                let hash = Sha256::digest(&bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                installed.push(Installed {
                    manifest,
                    hash,
                    folder: folder.clone(),
                });
            }
        }
    }
    (installed, rejected)
}

struct Record {
    trust: TrustLevel,
    hash: String,
}

fn records(db: &Connection) -> Result<BTreeMap<String, Record>, AtlasError> {
    let mut statement = db
        .prepare("SELECT plugin_id,trust,manifest_hash FROM plugin_state")
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(sql_error)?;
    let mut found = BTreeMap::new();
    for row in rows {
        let (id, trust, hash) = row.map_err(sql_error)?;
        // A trust value this version does not know is not a grant.
        if let Some(trust) = parse::<TrustLevel>(&trust) {
            found.insert(id, Record { trust, hash });
        }
    }
    Ok(found)
}

fn granted(
    db: &Connection,
    plugin: &str,
) -> Result<BTreeMap<String, BTreeSet<Capability>>, AtlasError> {
    let mut statement = db
        .prepare("SELECT g.server_id,g.capability FROM plugin_grant g JOIN servers s ON s.id=g.server_id WHERE g.plugin_id=? ORDER BY g.server_id")
        .map_err(sql_error)?;
    let rows = statement
        .query_map([plugin], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_error)?;
    let mut found: BTreeMap<String, BTreeSet<Capability>> = BTreeMap::new();
    for row in rows {
        let (server, capability) = row.map_err(sql_error)?;
        if let Some(capability) = parse::<Capability>(&capability) {
            found.entry(server).or_default().insert(capability);
        }
    }
    Ok(found)
}

/// The install record of a plugin, if InfraWerf installed it.
fn install_record(
    db: &Connection,
    plugin: &str,
) -> Result<Option<crate::plugin_install::InstallRecordView>, AtlasError> {
    use crate::plugin_install::{InstallRecordView, Integrity, SourceKind};
    let row = db
        .query_row(
            "SELECT source_kind,source,content_hash,archive_hash,signer_key_id,installed_at,integrity,checked_at FROM plugin_install WHERE plugin_id=?",
            [plugin],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .map(Some)
        .or_else(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(sql_error(other)),
        })?;
    Ok(row.map(
        |(kind, source, content, archive, signer, at, integrity, checked)| {
            InstallRecordView {
                source_kind: parse::<SourceKind>(&kind).unwrap_or(SourceKind::Folder),
                source,
                content_sha256: content,
                archive_sha256: archive,
                signer_key_id: signer,
                installed_at: at,
                // An unknown value is not proof the files are intact.
                integrity: parse::<Integrity>(&integrity).unwrap_or(Integrity::Changed),
                checked_at: checked,
            }
        },
    ))
}

fn files_changed(db: &Connection, plugin: &str) -> Result<bool, AtlasError> {
    Ok(install_record(db, plugin)?
        .is_some_and(|record| record.integrity == crate::plugin_install::Integrity::Changed))
}

fn status(installed: &Installed, record: Option<&Record>) -> PluginStatus {
    match record {
        None => PluginStatus::Disabled,
        Some(record) if record.hash == installed.hash => PluginStatus::Enabled,
        Some(_) => PluginStatus::ChangedSinceReview,
    }
}

/// Whether a trust level admits a kind of capability, and whether the owner
/// must confirm each use.
pub fn trust_admits(trust: TrustLevel, kind: CapabilityKind) -> Result<bool, Refusal> {
    match (kind, trust) {
        (CapabilityKind::Read, _) => Ok(false),
        (CapabilityKind::Change, TrustLevel::ReadOnly) => Err(Refusal::TrustTooLow),
        (CapabilityKind::Change, TrustLevel::ConfirmChanges) => Ok(true),
        (CapabilityKind::Change, TrustLevel::TrustedAutomation) => Ok(false),
        // Trust never implies sudo; that takes a named exception (T042).
        (CapabilityKind::Elevation, _) => Err(Refusal::NoElevationException),
    }
}

fn server_exists(db: &Connection, server: &ServerId) -> Result<bool, AtlasError> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM servers WHERE id=?)",
        [server.as_str()],
        |row| row.get(0),
    )
    .map_err(sql_error)
}

fn server_ids(db: &Connection) -> Result<Vec<ServerId>, AtlasError> {
    let mut statement = db
        .prepare("SELECT id FROM servers ORDER BY created_at,id")
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    rows.iter().map(|id| ServerId::parse(id)).collect()
}

fn all_off(db: &Connection) -> Result<bool, AtlasError> {
    db.query_row(
        "SELECT COALESCE((SELECT all_off FROM plugin_switch WHERE id=1),0)",
        [],
        |row| row.get(0),
    )
    .map_err(sql_error)
}

/// Appends one entry and keeps the log within the history retention bound.
/// Callers pass identifiers and fixed words only.
fn record(
    db: &Connection,
    plugin: (&str, &str),
    server: Option<&str>,
    action: &str,
    outcome: ActivityOutcome,
    detail: &str,
) -> Result<(), AtlasError> {
    db.execute(
        "INSERT INTO plugin_activity(at,plugin_id,plugin_name,server_id,action,outcome,detail) VALUES(?,?,?,?,?,?,?)",
        params![Utc::now().to_rfc3339(), plugin.0, plugin.1, server, action, name(outcome), detail],
    )
    .map_err(sql_error)?;
    db.execute(
        "DELETE FROM plugin_activity WHERE id NOT IN (SELECT id FROM plugin_activity ORDER BY id DESC LIMIT (SELECT history_limit FROM monitoring_settings WHERE id=1))",
        [],
    )
    .map_err(sql_error)?;
    Ok(())
}

fn decide(
    db: &Connection,
    installed: &[Installed],
    plugin: &str,
    server: &ServerId,
    capability: Capability,
) -> Result<Result<bool, Refusal>, AtlasError> {
    let Some(found) = installed.iter().find(|p| p.manifest.id == plugin) else {
        return Ok(Err(Refusal::UnknownPlugin));
    };
    if all_off(db)? {
        return Ok(Err(Refusal::AllPluginsOff));
    }
    if files_changed(db, plugin)? {
        return Ok(Err(Refusal::FilesChanged));
    }
    let all = records(db)?;
    let trust = match (status(found, all.get(plugin)), all.get(plugin)) {
        (PluginStatus::Enabled, Some(record)) => record.trust,
        (PluginStatus::ChangedSinceReview, _) => return Ok(Err(Refusal::ChangedSinceReview)),
        _ => return Ok(Err(Refusal::PluginDisabled)),
    };
    if !server_exists(db, server)? {
        return Ok(Err(Refusal::UnknownServer));
    }
    if !found.manifest.capabilities.contains(&capability) {
        return Ok(Err(Refusal::NotRequested));
    }
    if capability_kind(capability) == CapabilityKind::Elevation {
        return Ok(Err(Refusal::NoElevationException));
    }
    if !granted(db, plugin)?
        .get(server.as_str())
        .is_some_and(|set| set.contains(&capability))
    {
        return Ok(Err(Refusal::NotGranted));
    }
    Ok(trust_admits(trust, capability_kind(capability)))
}

/// Whether the owner's settings admit every listed capability for a plugin
/// on a server right now. `owner_starts` is for something that happens only
/// when the owner presses its button: that press is the confirmation a
/// "confirm each change" plugin needs. Nothing is logged.
fn admits(
    db: &Connection,
    installed: &[Installed],
    plugin: &str,
    server: &ServerId,
    capabilities: &[Capability],
    owner_starts: bool,
) -> Result<bool, AtlasError> {
    for capability in capabilities {
        match decide(db, installed, plugin, server, *capability)? {
            Ok(false) => {}
            Ok(true) if owner_starts => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn app_view(
    plugin: &PluginManifest,
    entry: &crate::plugin::ManifestContribution,
) -> Option<PackAppView> {
    let app = entry.app.as_ref()?;
    Some(PackAppView {
        plugin_id: plugin.id.clone(),
        plugin_name: plugin.name.clone(),
        contribution_id: entry.id.clone(),
        title: entry.title.clone(),
        port: app.port,
        path: app.path.clone().unwrap_or_else(|| "/".into()),
    })
}

/// A folder name inside the plugins directory: one plain name.
fn plugin_folder(directory: &Path, folder: &str) -> Result<std::path::PathBuf, AtlasError> {
    if folder.is_empty()
        || folder.starts_with('.')
        || folder.contains(['/', '\\', ':'])
        || folder.len() > 255
    {
        return Err(invalid());
    }
    let path = directory.join(folder);
    let meta =
        std::fs::symlink_metadata(&path).map_err(|_| AtlasError::new(ErrorCode::NotFound))?;
    if !meta.is_dir() {
        return Err(AtlasError::new(ErrorCode::NotFound));
    }
    Ok(path)
}

impl Store {
    /// Plugin views that may open on a server right now: the plugin is
    /// enabled, unchanged and intact, and the owner allowed every capability
    /// the view's contribution lists on that server. Nothing is logged.
    pub fn plugin_views(
        &self,
        directory: &Path,
        server: &ServerId,
    ) -> Result<Vec<PluginViewSource>, AtlasError> {
        let (installed, _) = discover(directory);
        let mut found = Vec::new();
        for plugin in &installed {
            for entry in &plugin.manifest.contributions {
                let Some(view) = &entry.view else { continue };
                if admits(
                    &self.db,
                    &installed,
                    &plugin.manifest.id,
                    server,
                    &entry.capabilities,
                    false,
                )? {
                    found.push(PluginViewSource {
                        tab: PluginViewTab {
                            plugin_id: plugin.manifest.id.clone(),
                            plugin_name: plugin.manifest.name.clone(),
                            contribution_id: entry.id.clone(),
                            title: entry.title.clone(),
                        },
                        folder: directory.join(&plugin.folder),
                        entry: view.entry.clone(),
                    });
                }
            }
        }
        Ok(found)
    }

    /// Logs something a plugin view did: it opened, asked to run a command,
    /// or tried something that was blocked. `detail` is a fixed word and, for
    /// a block, the scheme and host involved; never page content.
    pub fn plugin_view_event(
        &mut self,
        directory: &Path,
        plugin: &str,
        server: &ServerId,
        action: &str,
        outcome: ActivityOutcome,
        detail: &str,
    ) -> Result<(), AtlasError> {
        let (installed, _) = discover(directory);
        let label = installed
            .iter()
            .find(|p| p.manifest.id == plugin)
            .map_or("Unknown plugin".to_owned(), |p| p.manifest.name.clone());
        let known = server_exists(&self.db, server)?;
        let detail: String = detail
            .chars()
            .filter(|c| !c.is_control())
            .take(160)
            .collect();
        record(
            &self.db,
            (plugin, &label),
            known.then(|| server.as_str()),
            action,
            outcome,
            &detail,
        )
    }

    /// Publishes a prepared plugin and records how it was installed. It is
    /// disabled: installing grants nothing. Refused if a plugin with the
    /// same ID is already there (updates are T046).
    pub fn plugin_publish(
        &mut self,
        directory: &Path,
        prepared: &crate::plugin_install::Prepared,
    ) -> Result<crate::plugin_install::InstallReport, AtlasError> {
        use crate::plugin_install::{InstallProblem, InstallProblemCode, InstallReport};
        let id = &prepared.manifest.id;
        let (installed, _) = discover(directory);
        let taken = installed.iter().any(|p| &p.manifest.id == id)
            || directory.join(id).exists()
            || install_record(&self.db, id)?.is_some();
        if taken {
            return Ok(InstallReport::refused(InstallProblem::new(
                InstallProblemCode::AlreadyInstalled,
                format!(
                    "A plugin with the ID {id} is already installed. Remove it first; updating a plugin is not available yet."
                ),
            )));
        }
        let target = match crate::plugin_install::publish(directory, prepared) {
            Ok(target) => target,
            Err(error) if error.code == ErrorCode::Conflict => {
                return Ok(InstallReport::refused(InstallProblem::new(
                    InstallProblemCode::AlreadyInstalled,
                    format!("A folder named {id} is already in the plugins folder."),
                )));
            }
            Err(error) => return Err(error),
        };
        let folder = target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let now = Utc::now().to_rfc3339();
        let saved = (|| -> Result<(), AtlasError> {
            let transaction = self.db.transaction().map_err(sql_error)?;
            transaction
                .execute(
                    "INSERT INTO plugin_install VALUES(?,?,?,?,?,?,?,?,?,?)",
                    params![
                        id,
                        folder,
                        name(prepared.source_kind),
                        prepared.source,
                        prepared.content_sha256,
                        prepared.archive_sha256,
                        prepared.signer_key_id,
                        now,
                        name(crate::plugin_install::Integrity::Verified),
                        now
                    ],
                )
                .map_err(sql_error)?;
            // A plugin installed again starts from nothing.
            transaction
                .execute("DELETE FROM plugin_state WHERE plugin_id=?", [id])
                .map_err(sql_error)?;
            transaction
                .execute("DELETE FROM plugin_grant WHERE plugin_id=?", [id])
                .map_err(sql_error)?;
            record(
                &transaction,
                (id, &prepared.manifest.name),
                None,
                ACTION_INSTALL,
                ActivityOutcome::Done,
                &name(prepared.source_kind),
            )?;
            transaction.commit().map_err(sql_error)
        })();
        if let Err(error) = saved {
            // Without its record the plugin is not installed.
            let _ = std::fs::remove_dir_all(&target);
            return Err(error);
        }
        Ok(InstallReport {
            contract_version: CONTRACT_VERSION,
            plugin_id: Some(id.clone()),
            plugin_name: Some(prepared.manifest.name.clone()),
            content_sha256: Some(prepared.content_sha256.clone()),
            problem: None,
        })
    }

    /// Removes a plugin folder: its files, its trust, its permissions on
    /// every server and its install record. The activity log keeps its
    /// history. The folder is first renamed, so it stops being a plugin at
    /// once even if deleting its files takes a moment or fails.
    pub fn plugin_remove(&mut self, directory: &Path, folder: &str) -> Result<(), AtlasError> {
        let path = plugin_folder(directory, folder)?;
        let (installed, _) = discover(directory);
        let found = installed.iter().find(|p| p.folder == folder);
        let id = found.map(|p| p.manifest.id.clone()).or_else(|| {
            self.db
                .query_row(
                    "SELECT plugin_id FROM plugin_install WHERE folder=?",
                    [folder],
                    |row| row.get::<_, String>(0),
                )
                .ok()
        });
        let label = found.map_or(folder.to_owned(), |p| p.manifest.name.clone());
        let parked = directory.join(format!(
            "{}{}",
            crate::plugin_install::REMOVING_PREFIX,
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::rename(&path, &parked).map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
        let transaction = self.db.transaction().map_err(sql_error)?;
        if let Some(id) = &id {
            for table in ["plugin_state", "plugin_grant", "plugin_install"] {
                transaction
                    .execute(&format!("DELETE FROM {table} WHERE plugin_id=?"), [id])
                    .map_err(sql_error)?;
            }
        }
        record(
            &transaction,
            (id.as_deref().unwrap_or(""), &label),
            None,
            ACTION_REMOVE,
            ActivityOutcome::Done,
            "",
        )?;
        transaction.commit().map_err(sql_error)?;
        // A folder that could not be deleted now is deleted at the next
        // start-up; it is no longer a plugin either way.
        let _ = std::fs::remove_dir_all(&parked);
        Ok(())
    }

    /// The start-up check: removes interrupted working folders and compares
    /// every installed plugin's files with the hash recorded at install.
    /// Returns the IDs whose files changed or are gone.
    pub fn plugins_verify(&mut self, directory: &Path) -> Result<Vec<String>, AtlasError> {
        use crate::plugin_install::{Integrity, clean_working_folders, folder_hash};
        clean_working_folders(directory);
        let rows: Vec<(String, String, String)> = {
            let mut statement = self
                .db
                .prepare("SELECT plugin_id,folder,content_hash FROM plugin_install")
                .map_err(sql_error)?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .map_err(sql_error)?
                .collect::<Result<_, _>>()
                .map_err(sql_error)?
        };
        let now = Utc::now().to_rfc3339();
        let mut changed = Vec::new();
        for (id, folder, hash) in rows {
            let actual = plugin_folder(directory, &folder)
                .ok()
                .and_then(|path| folder_hash(&path));
            let integrity = if actual.as_deref() == Some(hash.as_str()) {
                Integrity::Verified
            } else {
                changed.push(id.clone());
                Integrity::Changed
            };
            self.db
                .execute(
                    "UPDATE plugin_install SET integrity=?,checked_at=? WHERE plugin_id=?",
                    params![name(integrity), now, id],
                )
                .map_err(sql_error)?;
        }
        Ok(changed)
    }

    /// What an update would change for an installed plugin: the installed
    /// manifest's hash (to notice a change before it applies) and the
    /// difference, including the previous and new signer.
    pub fn plugin_update_diff(
        &self,
        directory: &Path,
        plugin: &str,
        prepared: &crate::plugin_install::Prepared,
    ) -> Result<
        Result<(String, crate::plugin_install::UpdateDiff), crate::plugin_install::InstallProblem>,
        AtlasError,
    > {
        use crate::plugin_install::{InstallProblem, InstallProblemCode};
        let (installed, _) = discover(directory);
        let Some(current) = installed.iter().find(|p| p.manifest.id == plugin) else {
            return Err(AtlasError::new(ErrorCode::NotFound));
        };
        if prepared.manifest.id != plugin {
            return Ok(Err(InstallProblem::new(
                InstallProblemCode::DifferentPlugin,
                format!(
                    "This is the plugin {}, not {plugin}. An update must have the same ID.",
                    prepared.manifest.id
                ),
            )));
        }
        let mut diff = crate::plugin_install::diff(&current.manifest, &prepared.manifest);
        diff.previous_signer = install_record(&self.db, plugin)?.and_then(|r| r.signer_key_id);
        diff.signer = prepared.signer_key_id.clone();
        diff.content_sha256 = prepared.content_sha256.clone();
        diff.archive_sha256 = prepared.archive_sha256.clone();
        Ok(Ok((current.hash.clone(), diff)))
    }

    /// Applies an update the owner saw. The installed plugin must still be
    /// the one compared (`previous_hash`). The old folder stays until the
    /// new one is in place and its records are saved; any failure puts it
    /// back. An update that asks for nothing new keeps the owner's review;
    /// anything else leaves the plugin switched off until it is reviewed
    /// again. Permissions for capabilities the update no longer requests
    /// are removed; none are ever added.
    pub fn plugin_apply_update(
        &mut self,
        directory: &Path,
        plugin: &str,
        prepared: &crate::plugin_install::Prepared,
        previous_hash: &str,
        allow_downgrade: bool,
    ) -> Result<
        Result<crate::plugin_install::UpdateDiff, crate::plugin_install::InstallProblem>,
        AtlasError,
    > {
        use crate::plugin_install::{InstallProblem, InstallProblemCode, Integrity};
        let (hash, diff) = match self.plugin_update_diff(directory, plugin, prepared)? {
            Ok(found) => found,
            Err(problem) => return Ok(Err(problem)),
        };
        let (installed, _) = discover(directory);
        let current = installed
            .iter()
            .find(|p| p.manifest.id == plugin)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        if hash != previous_hash {
            return Ok(Err(InstallProblem::new(
                InstallProblemCode::ChangedSincePreview,
                "The installed plugin changed after you checked this update. Check the update again.",
            )));
        }
        if diff.downgrade && !allow_downgrade {
            return Ok(Err(InstallProblem::new(
                InstallProblemCode::Downgrade,
                format!(
                    "Version {} is older than the installed {}. Confirm the downgrade to apply it.",
                    diff.to_version, diff.from_version
                ),
            )));
        }
        let folder = current.folder.clone();
        let previous_record = install_record(&self.db, plugin)?;
        let files_were_changed = previous_record
            .as_ref()
            .is_some_and(|r| r.integrity == Integrity::Changed);
        let parked = crate::plugin_install::replace(directory, &folder, prepared)?;
        let new_hash = crate::plugin_install::manifest_hash(prepared);
        let keeps_review = !diff.needs_review && !files_were_changed;
        let now = Utc::now().to_rfc3339();
        let saved = (|| -> Result<(), AtlasError> {
            let transaction = self.db.transaction().map_err(sql_error)?;
            transaction
                .execute("DELETE FROM plugin_install WHERE plugin_id=?", [plugin])
                .map_err(sql_error)?;
            transaction
                .execute(
                    "INSERT INTO plugin_install VALUES(?,?,?,?,?,?,?,?,?,?)",
                    params![
                        plugin,
                        folder,
                        name(prepared.source_kind),
                        prepared.source,
                        prepared.content_sha256,
                        prepared.archive_sha256,
                        prepared.signer_key_id,
                        now,
                        name(Integrity::Verified),
                        now
                    ],
                )
                .map_err(sql_error)?;
            if keeps_review {
                // The owner already reviewed everything this version asks
                // for: the review carries over to its manifest.
                transaction
                    .execute(
                        "UPDATE plugin_state SET manifest_hash=? WHERE plugin_id=? AND manifest_hash=?",
                        params![new_hash, plugin, hash],
                    )
                    .map_err(sql_error)?;
            } else if files_were_changed {
                transaction
                    .execute("DELETE FROM plugin_state WHERE plugin_id=?", [plugin])
                    .map_err(sql_error)?;
            }
            // Otherwise the recorded review is for the old manifest, so the
            // plugin shows as changed since review and stays off.
            for capability in &diff.removed_capabilities {
                transaction
                    .execute(
                        "DELETE FROM plugin_grant WHERE plugin_id=? AND capability=?",
                        params![plugin, name(*capability)],
                    )
                    .map_err(sql_error)?;
            }
            let detail = format!(
                "{} to {}{}{}",
                diff.from_version,
                diff.to_version,
                if diff.downgrade { ", downgrade" } else { "" },
                if keeps_review {
                    ""
                } else {
                    ", needs your review"
                }
            );
            record(
                &transaction,
                (plugin, &prepared.manifest.name),
                None,
                ACTION_UPDATE,
                ActivityOutcome::Done,
                &detail,
            )?;
            transaction.commit().map_err(sql_error)
        })();
        match saved {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&parked);
                Ok(Ok(diff))
            }
            Err(error) => {
                crate::plugin_install::restore(directory, &folder, &parked);
                Err(error)
            }
        }
    }

    /// Logs an update that was refused or failed. The installed version is
    /// unchanged.
    pub fn plugin_update_failed(
        &mut self,
        directory: &Path,
        plugin: &str,
        why: &str,
    ) -> Result<(), AtlasError> {
        let (installed, _) = discover(directory);
        let label = installed
            .iter()
            .find(|p| p.manifest.id == plugin)
            .map_or(plugin.to_owned(), |p| p.manifest.name.clone());
        record(
            &self.db,
            (plugin, &label),
            None,
            ACTION_UPDATE,
            ActivityOutcome::Failed,
            why,
        )
    }

    /// Every pack badge to draw: the plugin is enabled and unchanged, and
    /// on that server the owner allowed both the badge and the command
    /// whose observation it shows. Nothing is logged.
    pub fn pack_badges(&self, directory: &Path) -> Result<Vec<PackBadgeBinding>, AtlasError> {
        let (installed, _) = discover(directory);
        let servers = server_ids(&self.db)?;
        let mut found = Vec::new();
        for plugin in &installed {
            let manifest = &plugin.manifest;
            for entry in &manifest.contributions {
                let Some(badge) = &entry.badge else { continue };
                let Some(source) = manifest
                    .contributions
                    .iter()
                    .find(|other| other.id == badge.observation && other.command.is_some())
                else {
                    continue;
                };
                for server in &servers {
                    if admits(
                        &self.db,
                        &installed,
                        &manifest.id,
                        server,
                        &entry.capabilities,
                        false,
                    )? && admits(
                        &self.db,
                        &installed,
                        &manifest.id,
                        server,
                        &source.capabilities,
                        false,
                    )? {
                        found.push(PackBadgeBinding {
                            server_id: server.clone(),
                            plugin_id: manifest.id.clone(),
                            plugin_name: manifest.name.clone(),
                            contribution_id: entry.id.clone(),
                            title: entry.title.clone(),
                            observation: source.id.clone(),
                        });
                    }
                }
            }
        }
        Ok(found)
    }

    /// Relationship kinds from enabled plugins, each with the servers the
    /// owner allowed it on. Nothing is logged.
    pub fn relationship_kinds(
        &self,
        directory: &Path,
    ) -> Result<Vec<PackRelationshipKindView>, AtlasError> {
        use crate::plugin::{KindColor, LineStyle};
        let (installed, _) = discover(directory);
        let servers = server_ids(&self.db)?;
        let mut found = Vec::new();
        for plugin in &installed {
            let manifest = &plugin.manifest;
            for entry in &manifest.contributions {
                if entry.point != crate::extensions::ExtensionPoint::RelationshipKind {
                    continue;
                }
                let mut allowed = Vec::new();
                for server in &servers {
                    if admits(
                        &self.db,
                        &installed,
                        &manifest.id,
                        server,
                        &entry.capabilities,
                        false,
                    )? {
                        allowed.push(server.clone());
                    }
                }
                if allowed.is_empty() {
                    continue;
                }
                let style = entry.relationship.as_ref();
                found.push(PackRelationshipKindView {
                    plugin_id: manifest.id.clone(),
                    plugin_name: manifest.name.clone(),
                    contribution_id: entry.id.clone(),
                    title: entry.title.clone(),
                    line: style.and_then(|s| s.line).unwrap_or(LineStyle::Solid),
                    color: style.and_then(|s| s.color).unwrap_or(KindColor::Green),
                    server_ids: allowed,
                });
            }
        }
        Ok(found)
    }

    /// Documents a relationship. A plugin kind is accepted only if its
    /// plugin is enabled and the owner allowed it on both servers; the
    /// relationship itself is the owner's map entry and applies nothing.
    pub fn add_relationship_with_kinds(
        &mut self,
        directory: &Path,
        input: AddRelationship,
    ) -> Result<EdgeId, AtlasError> {
        let Some(choice) = input.kind.clone() else {
            return self.add_relationship(input);
        };
        let kind = self
            .relationship_kinds(directory)?
            .into_iter()
            .find(|kind| {
                kind.plugin_id == choice.plugin_id
                    && kind.contribution_id == choice.contribution_id
                    && kind.server_ids.contains(&input.source)
                    && kind.server_ids.contains(&input.target)
            })
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        self.insert_relationship(
            input,
            Some((&kind.plugin_id, &kind.contribution_id, &kind.title)),
        )
    }

    /// Every pack application the owner allowed on a server. Opening a
    /// tunnel is a change, so a read-only plugin's applications are not
    /// listed. Nothing is logged.
    pub fn pack_apps(
        &self,
        directory: &Path,
        server: &ServerId,
    ) -> Result<Vec<PackAppView>, AtlasError> {
        let (installed, _) = discover(directory);
        let mut found = Vec::new();
        for plugin in &installed {
            for entry in &plugin.manifest.contributions {
                let Some(view) = app_view(&plugin.manifest, entry) else {
                    continue;
                };
                if admits(
                    &self.db,
                    &installed,
                    &plugin.manifest.id,
                    server,
                    &entry.capabilities,
                    true,
                )? {
                    found.push(view);
                }
            }
        }
        Ok(found)
    }

    /// The check before a pack application is opened. The manifest is read
    /// once, so the port and path returned are the ones authorised. Each
    /// capability is decided and logged; the first refusal ends it. The
    /// owner pressing Open is the confirmation a "confirm each change"
    /// plugin needs.
    pub fn plugin_begin_app(
        &mut self,
        directory: &Path,
        plugin: &str,
        server: &ServerId,
        contribution: &str,
    ) -> Result<Result<PackAppView, Refusal>, AtlasError> {
        let (installed, _) = discover(directory);
        let Some(found) = installed.iter().find(|p| p.manifest.id == plugin) else {
            record(
                &self.db,
                ("", "Unknown plugin"),
                None,
                ACTION_OPEN_APP,
                ActivityOutcome::Refused,
                &name(Refusal::UnknownPlugin),
            )?;
            return Ok(Err(Refusal::UnknownPlugin));
        };
        let entry = found
            .manifest
            .contributions
            .iter()
            .find(|c| c.id == contribution)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let view =
            app_view(&found.manifest, entry).ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let label = (found.manifest.id.as_str(), found.manifest.name.as_str());
        let known = server_exists(&self.db, server)?;
        for capability in &entry.capabilities {
            let outcome = decide(&self.db, &installed, plugin, server, *capability)?;
            record(
                &self.db,
                label,
                known.then(|| server.as_str()),
                &name(*capability),
                if outcome.is_ok() {
                    ActivityOutcome::Allowed
                } else {
                    ActivityOutcome::Refused
                },
                &outcome.err().map(name).unwrap_or_default(),
            )?;
            if let Err(refusal) = outcome {
                return Ok(Err(refusal));
            }
        }
        Ok(Ok(view))
    }

    /// Log whether a pack application's tunnel opened.
    pub fn plugin_finish_app(
        &mut self,
        app: &PackAppView,
        server: &ServerId,
        outcome: ActivityOutcome,
    ) -> Result<(), AtlasError> {
        record(
            &self.db,
            (&app.plugin_id, &app.plugin_name),
            server_exists(&self.db, server)?.then(|| server.as_str()),
            ACTION_OPEN_APP,
            outcome,
            &app.contribution_id,
        )
    }

    /// Every plugin found, with what it asks for and what the owner allowed.
    pub fn plugins_state(&self, directory: &Path) -> Result<PluginsState, AtlasError> {
        let (installed, invalid) = discover(directory);
        let all = records(&self.db)?;
        let mut plugins = Vec::new();
        for found in &installed {
            let manifest = &found.manifest;
            let record = all.get(&manifest.id);
            let install = install_record(&self.db, &manifest.id)?;
            let state = if install
                .as_ref()
                .is_some_and(|i| i.integrity == crate::plugin_install::Integrity::Changed)
            {
                PluginStatus::FilesChanged
            } else {
                status(found, record)
            };
            plugins.push(PluginView {
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                api_version: manifest.api_version,
                description: manifest.description.clone(),
                author: manifest.author.clone(),
                license: manifest.license.clone(),
                homepage: manifest.homepage.clone(),
                folder: found.folder.clone(),
                status: state,
                trust: record
                    .filter(|_| state == PluginStatus::Enabled)
                    .map(|record| record.trust),
                capabilities: manifest
                    .capabilities
                    .iter()
                    .map(|capability| RequestedCapability {
                        capability: *capability,
                        kind: capability_kind(*capability),
                        summary: capability_summary(*capability).into(),
                        used_by: manifest
                            .contributions
                            .iter()
                            .filter(|c| c.capabilities.contains(capability))
                            .map(|c| c.title.clone())
                            .collect(),
                    })
                    .collect(),
                contributions: manifest
                    .contributions
                    .iter()
                    .map(|c| PluginContributionView {
                        id: c.id.clone(),
                        title: c.title.clone(),
                        point: c.point,
                        capabilities: c.capabilities.clone(),
                        command: c.command.clone(),
                        workload_kind: c.workload_kind.clone(),
                        app: c.app.clone(),
                        badge: c.badge.clone(),
                        relationship: c.relationship.clone(),
                        view: c.view.clone(),
                    })
                    .collect(),
                grants: granted(&self.db, &manifest.id)?
                    .into_iter()
                    .filter_map(|(server, capabilities)| {
                        Some(ServerGrant {
                            server_id: ServerId::parse(&server).ok()?,
                            // Only what the current manifest still requests.
                            capabilities: capabilities
                                .into_iter()
                                .filter(|c| manifest.capabilities.contains(c))
                                .collect(),
                        })
                    })
                    .filter(|grant| !grant.capabilities.is_empty())
                    .collect(),
                install,
            });
        }
        Ok(PluginsState {
            contract_version: CONTRACT_VERSION,
            directory: directory.to_string_lossy().into_owned(),
            all_off: all_off(&self.db)?,
            plugins,
            invalid,
        })
    }

    /// The newest entries of the plugin activity log.
    pub fn plugin_activity(&self, limit: u32) -> Result<PluginActivityLog, AtlasError> {
        let kept: u32 = self
            .db
            .query_row(
                "SELECT history_limit FROM monitoring_settings WHERE id=1",
                [],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        let mut statement = self
            .db
            .prepare("SELECT id,at,plugin_id,plugin_name,server_id,action,outcome,detail FROM plugin_activity ORDER BY id DESC LIMIT ?")
            .map_err(sql_error)?;
        let rows = statement
            .query_map([limit.clamp(1, MAX_ACTIVITY_PAGE)], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(sql_error)?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, at, plugin_id, plugin_name, server, action, outcome, detail) =
                row.map_err(sql_error)?;
            let Some(outcome) = parse::<ActivityOutcome>(&outcome) else {
                continue;
            };
            entries.push(PluginActivity {
                id,
                at,
                plugin_id,
                plugin_name,
                server_id: server.and_then(|server| ServerId::parse(&server).ok()),
                action,
                outcome,
                detail,
            });
        }
        Ok(PluginActivityLog {
            contract_version: CONTRACT_VERSION,
            kept,
            entries,
        })
    }

    /// The kill switch. While on, every plugin call is refused whatever
    /// was granted. Turning it off again restores what the owner had set.
    pub fn plugins_set_all_off(&mut self, off: bool) -> Result<(), AtlasError> {
        if all_off(&self.db)? == off {
            return Ok(());
        }
        self.db
            .execute(
                "INSERT INTO plugin_switch(id,all_off) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET all_off=?1",
                [off],
            )
            .map_err(sql_error)?;
        record(
            &self.db,
            ("", "All plugins"),
            None,
            if off { ACTION_ALL_OFF } else { ACTION_ALL_ON },
            ActivityOutcome::Done,
            "",
        )
    }

    /// Takes trusted automation away from a plugin without disabling it:
    /// from now on each change needs the owner's confirmation.
    pub fn plugin_revoke_trusted(
        &mut self,
        directory: &Path,
        plugin: &str,
    ) -> Result<(), AtlasError> {
        let changed = self
            .db
            .execute(
                "UPDATE plugin_state SET trust=? WHERE plugin_id=? AND trust=?",
                params![
                    name(TrustLevel::ConfirmChanges),
                    plugin,
                    name(TrustLevel::TrustedAutomation)
                ],
            )
            .map_err(sql_error)?;
        if changed == 0 {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        let (installed, _) = discover(directory);
        let label = installed
            .iter()
            .find(|p| p.manifest.id == plugin)
            .map_or("A removed plugin", |p| p.manifest.name.as_str());
        record(
            &self.db,
            (plugin, label),
            None,
            ACTION_REVOKE_TRUSTED,
            ActivityOutcome::Done,
            &name(TrustLevel::ConfirmChanges),
        )
    }

    /// Enables a plugin at the trust level the owner chose, recording the
    /// manifest that was reviewed.
    pub fn plugin_enable(
        &mut self,
        directory: &Path,
        plugin: &str,
        trust: TrustLevel,
    ) -> Result<(), AtlasError> {
        let (installed, _) = discover(directory);
        let found = installed
            .iter()
            .find(|p| p.manifest.id == plugin)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        self.db
            .execute(
                "INSERT INTO plugin_state(plugin_id,trust,manifest_hash,enabled_at) VALUES(?,?,?,?)
                 ON CONFLICT(plugin_id) DO UPDATE SET trust=excluded.trust,manifest_hash=excluded.manifest_hash,enabled_at=excluded.enabled_at",
                params![plugin, name(trust), found.hash, Utc::now().to_rfc3339()],
            )
            .map_err(sql_error)?;
        record(
            &self.db,
            (plugin, &found.manifest.name),
            None,
            ACTION_ENABLE,
            ActivityOutcome::Done,
            &name(trust),
        )
    }

    /// Disables a plugin at once. Its trust level is forgotten, so enabling
    /// it again needs a new choice. Server permissions are kept but unused.
    pub fn plugin_disable(&mut self, directory: &Path, plugin: &str) -> Result<(), AtlasError> {
        let removed = self
            .db
            .execute("DELETE FROM plugin_state WHERE plugin_id=?", [plugin])
            .map_err(sql_error)?;
        if removed == 0 {
            return Err(AtlasError::new(ErrorCode::NotFound));
        }
        let (installed, _) = discover(directory);
        let label = installed
            .iter()
            .find(|p| p.manifest.id == plugin)
            .map_or("A removed plugin", |p| p.manifest.name.as_str());
        record(
            &self.db,
            (plugin, label),
            None,
            ACTION_DISABLE,
            ActivityOutcome::Done,
            "",
        )
    }

    /// Replaces what a plugin may use on one server. An empty list withdraws
    /// everything there. Only capabilities the manifest requests can be
    /// granted, and sudo cannot be granted this way.
    pub fn plugin_set_grant(
        &mut self,
        directory: &Path,
        plugin: &str,
        server: &ServerId,
        capabilities: &[Capability],
    ) -> Result<(), AtlasError> {
        let (installed, _) = discover(directory);
        let found = installed
            .iter()
            .find(|p| p.manifest.id == plugin)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let unique: BTreeSet<Capability> = capabilities.iter().copied().collect();
        if unique.len() != capabilities.len()
            || unique.iter().any(|capability| {
                !found.manifest.capabilities.contains(capability)
                    || capability_kind(*capability) == CapabilityKind::Elevation
            })
        {
            return Err(invalid());
        }
        if !server_exists(&self.db, server)? {
            return Err(AtlasError::new(ErrorCode::NotFound));
        }
        let transaction = self.db.transaction().map_err(sql_error)?;
        transaction
            .execute(
                "DELETE FROM plugin_grant WHERE plugin_id=? AND server_id=?",
                params![plugin, server.as_str()],
            )
            .map_err(sql_error)?;
        for capability in &unique {
            transaction
                .execute(
                    "INSERT INTO plugin_grant(plugin_id,server_id,capability) VALUES(?,?,?)",
                    params![plugin, server.as_str(), name(*capability)],
                )
                .map_err(sql_error)?;
        }
        let detail = unique
            .iter()
            .map(|capability| name(*capability))
            .collect::<Vec<_>>()
            .join(", ");
        record(
            &transaction,
            (plugin, &found.manifest.name),
            Some(server.as_str()),
            ACTION_SET_PERMISSIONS,
            ActivityOutcome::Done,
            &detail,
        )?;
        transaction.commit().map_err(sql_error)
    }

    /// Every pack command that may run on a server right now: the plugin is
    /// enabled and unchanged, plugins are not all off, and the owner allowed
    /// each capability the command uses on this server. Nothing is logged.
    pub fn pack_commands(
        &self,
        directory: &Path,
        server: &ServerId,
    ) -> Result<Vec<PackRun>, AtlasError> {
        let (installed, _) = discover(directory);
        let mut found = Vec::new();
        for plugin in &installed {
            for contribution in &plugin.manifest.contributions {
                let Some(command) = &contribution.command else {
                    continue;
                };
                let mut admitted = true;
                for capability in &contribution.capabilities {
                    admitted &= decide(
                        &self.db,
                        &installed,
                        &plugin.manifest.id,
                        server,
                        *capability,
                    )? == Ok(false);
                }
                if admitted {
                    found.push(PackRun {
                        plugin_id: plugin.manifest.id.clone(),
                        plugin_name: plugin.manifest.name.clone(),
                        contribution: contribution.clone(),
                        command: command.clone(),
                    });
                }
            }
        }
        Ok(found)
    }

    /// The check before one run of a pack command (T043). The manifest is
    /// read once, so the command returned is the one that was authorised.
    /// Each capability the command uses is decided and logged exactly as
    /// `plugin_authorize` does; the first refusal ends the run.
    pub fn plugin_begin_run(
        &mut self,
        directory: &Path,
        plugin: &str,
        server: &ServerId,
        contribution: &str,
    ) -> Result<Result<PackRun, Refusal>, AtlasError> {
        let (installed, _) = discover(directory);
        let Some(found) = installed.iter().find(|p| p.manifest.id == plugin) else {
            record(
                &self.db,
                ("", "Unknown plugin"),
                None,
                ACTION_RUN_COMMAND,
                ActivityOutcome::Refused,
                &name(Refusal::UnknownPlugin),
            )?;
            return Ok(Err(Refusal::UnknownPlugin));
        };
        let entry = found
            .manifest
            .contributions
            .iter()
            .find(|c| c.id == contribution)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let command = entry
            .command
            .clone()
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let label = (found.manifest.id.as_str(), found.manifest.name.as_str());
        let known = server_exists(&self.db, server)?;
        for capability in &entry.capabilities {
            let outcome = decide(&self.db, &installed, plugin, server, *capability)?;
            // A read-only command never waits for a confirmation; a
            // capability that would need one is not a read and is refused.
            let outcome = match outcome {
                Ok(false) => Ok(()),
                Ok(true) => Err(Refusal::TrustTooLow),
                Err(refusal) => Err(refusal),
            };
            record(
                &self.db,
                label,
                known.then(|| server.as_str()),
                &name(*capability),
                if outcome.is_ok() {
                    ActivityOutcome::Allowed
                } else {
                    ActivityOutcome::Refused
                },
                &outcome.err().map(name).unwrap_or_default(),
            )?;
            if let Err(refusal) = outcome {
                return Ok(Err(refusal));
            }
        }
        Ok(Ok(PackRun {
            plugin_id: found.manifest.id.clone(),
            plugin_name: found.manifest.name.clone(),
            contribution: entry.clone(),
            command,
        }))
    }

    /// Log how a pack command's run ended: done, cancelled or failed.
    pub fn plugin_finish_run(
        &mut self,
        run: &PackRun,
        server: &ServerId,
        outcome: ActivityOutcome,
    ) -> Result<(), AtlasError> {
        record(
            &self.db,
            (&run.plugin_id, &run.plugin_name),
            server_exists(&self.db, server)?.then(|| server.as_str()),
            ACTION_RUN_COMMAND,
            outcome,
            &run.contribution.id,
        )
    }

    /// The enforcement point. Admits a plugin's use of one capability on one
    /// server only if the plugin is enabled as reviewed, its manifest
    /// requested the capability, the owner granted it on that server and
    /// the chosen trust level allows it. Nothing is cached: each call is
    /// decided against what the owner allows now, so a withdrawal, a
    /// revocation or the kill switch takes effect on the very next call.
    /// Every answer is logged.
    pub fn plugin_authorize(
        &mut self,
        directory: &Path,
        plugin: &str,
        server: &ServerId,
        capability: Capability,
    ) -> Result<PluginDecision, AtlasError> {
        let (installed, _) = discover(directory);
        let outcome = decide(&self.db, &installed, plugin, server, capability)?;
        // Only a real plugin's ID and name are written to the log; whatever
        // an unknown caller sent as its ID is not.
        let label = installed
            .iter()
            .find(|p| p.manifest.id == plugin)
            .map_or(("", "Unknown plugin"), |p| {
                (p.manifest.id.as_str(), p.manifest.name.as_str())
            });
        let known = server_exists(&self.db, server)?;
        let (result, detail) = match outcome {
            Ok(false) => (ActivityOutcome::Allowed, String::new()),
            Ok(true) => (ActivityOutcome::NeedsConfirmation, String::new()),
            Err(refusal) => (ActivityOutcome::Refused, name(refusal)),
        };
        record(
            &self.db,
            label,
            known.then(|| server.as_str()),
            &name(capability),
            result,
            &detail,
        )?;
        Ok(PluginDecision {
            contract_version: CONTRACT_VERSION,
            allowed: outcome.is_ok(),
            needs_confirmation: outcome == Ok(true),
            refusal: outcome.err(),
        })
    }
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        TrustLevel::decl(config),
        CapabilityKind::decl(config),
        PluginStatus::decl(config),
        Refusal::decl(config),
        PluginDecision::decl(config),
        RequestedCapability::decl(config),
        PluginContributionView::decl(config),
        ServerGrant::decl(config),
        PluginView::decl(config),
        InvalidPlugin::decl(config),
        PluginsState::decl(config),
        ActivityOutcome::decl(config),
        PluginActivity::decl(config),
        PluginActivityLog::decl(config),
        PluginViewTab::decl(config),
        PluginViewsState::decl(config),
    ]
    .into_iter()
    .map(|d| format!("export {d}\n"))
    .collect()
}
