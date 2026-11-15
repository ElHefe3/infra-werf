//! Installed plugins, owner grants and the enforcement point (S12/F20/T041).
//!
//! A plugin is a folder in the plugins directory holding a valid manifest.
//! Finding one grants nothing. The owner enables a plugin by choosing a
//! trust level (nothing is preselected) and grants capabilities per server,
//! always from the plugin's own page. `authorize` is the one place a plugin
//! call is admitted or refused; it runs here in the core, never in the
//! interface, and every refusal is written to the activity history.
//!
//! No grant reaches keys, passwords or an unrestricted shell: the manifest
//! cannot request the interactive terminal, and elevation is refused until
//! the owner can name an exception for one action (T042).
use crate::extensions::{Capability, ExtensionPoint};
use crate::plugin::{MANIFEST_FILE, MAX_MANIFEST_BYTES, PluginManifest, validate_manifest};
use crate::store::{event, sql_error};
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
";

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
}

impl Refusal {
    fn sentence(self) -> &'static str {
        match self {
            Self::UnknownPlugin => "the plugin is not installed",
            Self::PluginDisabled => "the plugin is not enabled",
            Self::ChangedSinceReview => "the plugin changed after you reviewed it",
            Self::UnknownServer => "the server is not on your map",
            Self::NotRequested => "the plugin never asked for it",
            Self::NotGranted => "it is not granted on this server",
            Self::TrustTooLow => "the plugin is read-only",
            Self::NoElevationException => "sudo needs an exception you have not given",
        }
    }
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

impl Store {
    /// Every plugin found, with what it asks for and what the owner allowed.
    pub fn plugins_state(&self, directory: &Path) -> Result<PluginsState, AtlasError> {
        let (installed, invalid) = discover(directory);
        let all = records(&self.db)?;
        let mut plugins = Vec::new();
        for found in &installed {
            let manifest = &found.manifest;
            let record = all.get(&manifest.id);
            let state = status(found, record);
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
            });
        }
        Ok(PluginsState {
            contract_version: CONTRACT_VERSION,
            directory: directory.to_string_lossy().into_owned(),
            plugins,
            invalid,
        })
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
        let level = match trust {
            TrustLevel::ReadOnly => "read-only",
            TrustLevel::ConfirmChanges => "changes with confirmation",
            TrustLevel::TrustedAutomation => "trusted automation",
        };
        event(
            &self.db,
            &format!("Plugin \"{}\" enabled: {level}.", found.manifest.name),
            None,
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
        event(&self.db, &format!("Plugin \"{label}\" disabled."), None)
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
        let message = if unique.is_empty() {
            format!(
                "Plugin \"{}\": all permissions on this server withdrawn.",
                found.manifest.name
            )
        } else {
            format!(
                "Plugin \"{}\": permissions on this server set ({}).",
                found.manifest.name,
                unique
                    .iter()
                    .map(|capability| name(*capability))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        event(&transaction, &message, Some(server.as_str()))?;
        transaction.commit().map_err(sql_error)
    }

    /// The enforcement point. Admits a plugin's use of one capability on one
    /// server only if the plugin is enabled as reviewed, its manifest
    /// requested the capability, the owner granted it on that server and
    /// the chosen trust level allows it. A refusal is logged.
    pub fn plugin_authorize(
        &mut self,
        directory: &Path,
        plugin: &str,
        server: &ServerId,
        capability: Capability,
    ) -> Result<PluginDecision, AtlasError> {
        let (installed, _) = discover(directory);
        let outcome = decide(&self.db, &installed, plugin, server, capability)?;
        if let Err(refusal) = outcome {
            let label = installed
                .iter()
                .find(|p| p.manifest.id == plugin)
                .map_or_else(
                    || "An unknown plugin".to_owned(),
                    |p| format!("Plugin \"{}\"", p.manifest.name),
                );
            let known = server_exists(&self.db, server)?;
            event(
                &self.db,
                &format!(
                    "{label} was refused {}: {}.",
                    name(capability),
                    refusal.sentence()
                ),
                known.then(|| server.as_str()),
            )?;
        }
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
    ]
    .into_iter()
    .map(|d| format!("export {d}\n"))
    .collect()
}
