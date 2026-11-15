//! Plugin manifest and API version (S12/F19/T039).
//!
//! A plugin is described by one JSON file, `infrawerf-plugin.json`. This
//! module defines that file, validates it and publishes its JSON Schema from
//! the same types. Validation only reads the manifest: nothing a plugin
//! ships is executed here, and a valid manifest grants nothing. What the
//! manifest requests is shown to the owner, who decides (F20).
//!
//! Compatibility rule: `PLUGIN_API_VERSION` is one integer. It increases
//! whenever a plugin written for the previous version could behave
//! differently or a manifest could mean something else, including any new
//! manifest field, extension point or capability. InfraWerf accepts every
//! version from `MIN_PLUGIN_API_VERSION` to `PLUGIN_API_VERSION` and refuses
//! a newer one. See `docs/plugin-api/README.md`.
use crate::extensions::{Capability, Contribution, ExtensionPoint};
use crate::text;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// The plugin API this InfraWerf implements.
pub const PLUGIN_API_VERSION: u32 = 1;
/// The oldest plugin API this InfraWerf still accepts.
pub const MIN_PLUGIN_API_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "infrawerf-plugin.json";
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_PLUGIN_CONTRIBUTIONS: usize = 64;
/// First ID segments a plugin may not use.
pub const RESERVED_ID_PREFIXES: [&str; 2] = ["builtin", "infrawerf"];

/// The contents of `infrawerf-plugin.json` for plugin API v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "InfraWerf plugin manifest (API v1)")]
pub struct PluginManifest {
    /// Reverse-domain identifier, for example `org.example.backup-overview`:
    /// at least three lower-case segments of letters, digits and hyphens,
    /// each starting with a letter, 63 characters at most. It never changes
    /// between versions of the same plugin.
    pub id: String,
    /// Display name, 40 characters at most.
    pub name: String,
    /// The plugin's own version, as Semantic Versioning 2.0.0.
    pub version: String,
    /// The InfraWerf plugin API version the plugin was written for.
    pub api_version: u32,
    /// One-line description, 280 characters at most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Author or maintainer, 80 characters at most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Licence, preferably an SPDX identifier, 64 characters at most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// An `https://` address for the plugin, 200 characters at most.
    /// InfraWerf shows it and never fetches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// Every capability the plugin asks the owner for. Each must be used by
    /// at least one contribution.
    pub capabilities: Vec<Capability>,
    /// What the plugin adds, between 1 and 64 entries.
    pub contributions: Vec<ManifestContribution>,
}

/// One thing a plugin adds at an extension point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManifestContribution {
    /// Name within the plugin: lower-case letters, digits and hyphens,
    /// starting with a letter, 32 characters at most. The full identifier
    /// is the plugin ID, a dot, then this.
    pub id: String,
    pub point: ExtensionPoint,
    /// Display name, 40 characters at most.
    pub title: String,
    /// Position among contributions at the same point. Defaults to 1000,
    /// after everything built in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u16>,
    /// The capabilities this contribution uses: at least one, each
    /// requested by the manifest and permitted at this extension point.
    pub capabilities: Vec<Capability>,
}

pub const DEFAULT_PLUGIN_ORDER: u16 = 1000;

/// Why a manifest was refused. These codes are part of the plugin API: a
/// code keeps its meaning for as long as the API version it appeared in is
/// accepted.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManifestRule {
    /// The file is not valid JSON (or not UTF-8).
    MalformedJson,
    /// The file is larger than 64 KiB.
    TooLarge,
    /// A value has the wrong JSON type.
    WrongType,
    /// A field that is not part of this API version.
    UnknownField,
    /// A required field is absent.
    MissingField,
    /// The plugin ID is not a reverse-domain identifier.
    InvalidId,
    /// The plugin ID starts with a segment reserved for InfraWerf.
    ReservedId,
    /// The name is empty, too long or has surrounding or control characters.
    InvalidName,
    /// The version is not Semantic Versioning 2.0.0.
    InvalidVersion,
    /// The API version is not a positive whole number.
    InvalidApiVersion,
    /// The plugin needs a newer plugin API than this InfraWerf has.
    NewerApiVersion,
    /// A description, author or licence is empty, too long or malformed.
    InvalidText,
    /// The homepage is not a plain `https://` address.
    InvalidUrl,
    /// A capability name this API version does not have.
    UnknownCapability,
    /// The same capability is listed twice in one list.
    DuplicateCapability,
    /// A capability no plugin may request.
    CapabilityNotAvailable,
    /// A requested capability that no contribution uses.
    UnusedCapability,
    /// The plugin contributes nothing.
    NoContributions,
    /// More than 64 contributions.
    TooManyContributions,
    /// An extension point this API version does not have.
    UnknownPoint,
    /// A contribution ID is not a lower-case name.
    InvalidContributionId,
    /// Two contributions share an ID.
    DuplicateContribution,
    /// A title is empty, too long or has surrounding or control characters.
    InvalidTitle,
    /// A contribution lists no capability.
    ContributionWithoutCapability,
    /// A contribution uses a capability the manifest does not request.
    CapabilityNotRequested,
    /// A contribution uses a capability its extension point does not permit.
    CapabilityNotPermitted,
}

impl ManifestRule {
    pub const ALL: [ManifestRule; 26] = [
        Self::MalformedJson,
        Self::TooLarge,
        Self::WrongType,
        Self::UnknownField,
        Self::MissingField,
        Self::InvalidId,
        Self::ReservedId,
        Self::InvalidName,
        Self::InvalidVersion,
        Self::InvalidApiVersion,
        Self::NewerApiVersion,
        Self::InvalidText,
        Self::InvalidUrl,
        Self::UnknownCapability,
        Self::DuplicateCapability,
        Self::CapabilityNotAvailable,
        Self::UnusedCapability,
        Self::NoContributions,
        Self::TooManyContributions,
        Self::UnknownPoint,
        Self::InvalidContributionId,
        Self::DuplicateContribution,
        Self::InvalidTitle,
        Self::ContributionWithoutCapability,
        Self::CapabilityNotRequested,
        Self::CapabilityNotPermitted,
    ];

    /// The stable code, as written in output and documentation.
    pub fn code(self) -> String {
        match serde_json::to_value(self) {
            Ok(Value::String(code)) => code,
            _ => unreachable!("a manifest rule serialises as a string"),
        }
    }
}

/// One refused rule: which, where and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestIssue {
    pub code: ManifestRule,
    /// The field, for example `contributions[1].title`; empty for the file
    /// as a whole.
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ManifestIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let field = if self.field.is_empty() {
            "(manifest)"
        } else {
            &self.field
        };
        write!(f, "{}  {}  {}", self.code.code(), field, self.message)
    }
}

/// Capabilities a plugin may request. An interactive terminal is the owner's
/// own raw session and is never handed to a plugin (D055).
pub fn plugin_may_request(capability: Capability) -> bool {
    capability != Capability::InteractiveTerminal
}

struct Issues(Vec<ManifestIssue>);
impl Issues {
    fn add(&mut self, code: ManifestRule, field: impl Into<String>, message: impl Into<String>) {
        self.0.push(ManifestIssue {
            code,
            field: field.into(),
            message: message.into(),
        });
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Integer,
    Capabilities,
    Point,
    Contributions,
}
impl Kind {
    fn expects(self) -> &'static str {
        match self {
            Kind::Text | Kind::Point => "a string",
            Kind::Integer => "a whole number",
            Kind::Capabilities => "a list of capability names",
            Kind::Contributions => "a list of contributions",
        }
    }
}

/// Field name, JSON type and whether it is required. A test keeps these
/// tables equal to the published schema, which is generated from the types.
const MANIFEST_FIELDS: &[(&str, Kind, bool)] = &[
    ("id", Kind::Text, true),
    ("name", Kind::Text, true),
    ("version", Kind::Text, true),
    ("api_version", Kind::Integer, true),
    ("description", Kind::Text, false),
    ("author", Kind::Text, false),
    ("license", Kind::Text, false),
    ("homepage", Kind::Text, false),
    ("capabilities", Kind::Capabilities, true),
    ("contributions", Kind::Contributions, true),
];
const CONTRIBUTION_FIELDS: &[(&str, Kind, bool)] = &[
    ("id", Kind::Text, true),
    ("point", Kind::Point, true),
    ("title", Kind::Text, true),
    ("order", Kind::Integer, false),
    ("capabilities", Kind::Capabilities, true),
];

/// Field names and required flags, for the schema consistency test.
pub fn declared_fields() -> [Vec<(&'static str, bool)>; 2] {
    [MANIFEST_FIELDS, CONTRIBUTION_FIELDS].map(|fields| {
        fields
            .iter()
            .map(|(name, _, need)| (*name, *need))
            .collect()
    })
}

fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}.{name}")
    }
}

/// Checks names and JSON types so every structural problem is reported with
/// its field, before the typed model is built.
fn shape(value: &Value, path: &str, fields: &[(&str, Kind, bool)], issues: &mut Issues) {
    let Some(object) = value.as_object() else {
        issues.add(
            ManifestRule::WrongType,
            path,
            "Expected an object with named fields.",
        );
        return;
    };
    for name in object.keys() {
        if !fields.iter().any(|(known, _, _)| known == name) {
            issues.add(
                ManifestRule::UnknownField,
                join(path, name),
                format!("Plugin API v{PLUGIN_API_VERSION} has no field named \"{name}\"."),
            );
        }
    }
    for (name, kind, required) in fields {
        let field = join(path, name);
        let Some(found) = object.get(*name) else {
            if *required {
                issues.add(
                    ManifestRule::MissingField,
                    field,
                    format!("\"{name}\" is required."),
                );
            }
            continue;
        };
        // An optional field may be written as null, as the schema says.
        if !*required && found.is_null() {
            continue;
        }
        let wrong = |issues: &mut Issues, field: String| {
            issues.add(
                ManifestRule::WrongType,
                field,
                format!("Expected {}.", kind.expects()),
            )
        };
        match kind {
            Kind::Text => {
                if !found.is_string() {
                    wrong(issues, field);
                }
            }
            Kind::Integer if *name == "api_version" => {
                if !found.is_number() {
                    wrong(issues, field);
                } else if !found
                    .as_u64()
                    .is_some_and(|n| n >= 1 && n <= u64::from(u32::MAX))
                {
                    issues.add(
                        ManifestRule::InvalidApiVersion,
                        field,
                        "The API version is a positive whole number, for example 1.",
                    );
                }
            }
            Kind::Integer => {
                if !found.as_u64().is_some_and(|n| n <= u64::from(u16::MAX)) {
                    issues.add(
                        ManifestRule::WrongType,
                        field,
                        "Expected a whole number from 0 to 65535.",
                    );
                }
            }
            Kind::Point => match found.as_str() {
                None => wrong(issues, field),
                Some(point) => {
                    if serde_json::from_value::<ExtensionPoint>(found.clone()).is_err() {
                        issues.add(
                            ManifestRule::UnknownPoint,
                            field,
                            format!(
                                "Plugin API v{PLUGIN_API_VERSION} has no extension point named \"{point}\"."
                            ),
                        );
                    }
                }
            },
            Kind::Capabilities => match found.as_array() {
                None => wrong(issues, field),
                Some(list) => {
                    for (index, entry) in list.iter().enumerate() {
                        let item = format!("{field}[{index}]");
                        match entry.as_str() {
                            None => issues.add(
                                ManifestRule::WrongType,
                                item,
                                "Expected a capability name as a string.",
                            ),
                            Some(capability) => {
                                if serde_json::from_value::<Capability>(entry.clone()).is_err() {
                                    issues.add(
                                        ManifestRule::UnknownCapability,
                                        item,
                                        format!(
                                            "Plugin API v{PLUGIN_API_VERSION} has no capability named \"{capability}\"."
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            },
            Kind::Contributions => match found.as_array() {
                None => wrong(issues, field),
                Some(list) => {
                    for (index, entry) in list.iter().enumerate() {
                        shape(
                            entry,
                            &format!("{field}[{index}]"),
                            CONTRIBUTION_FIELDS,
                            issues,
                        );
                    }
                }
            },
        }
    }
}

fn slug(segment: &str) -> bool {
    segment
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_lowercase)
        && !segment.ends_with('-')
        && segment
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn plugin_id(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    id.len() <= 63 && segments.len() >= 3 && segments.iter().all(|segment| slug(segment))
}

/// Semantic Versioning 2.0.0: `MAJOR.MINOR.PATCH`, then optional
/// `-prerelease` and `+build`.
pub fn semantic_version(version: &str) -> bool {
    let numeric = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|b| b.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    let identifiers = |text: &str, numeric_rule: bool| {
        text.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && (!numeric_rule || !part.bytes().all(|b| b.is_ascii_digit()) || numeric(part))
        })
    };
    if version.len() > 64 {
        return false;
    }
    let (rest, build) = match version.split_once('+') {
        Some((rest, build)) => (rest, Some(build)),
        None => (version, None),
    };
    let (core, pre) = match rest.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (rest, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| numeric(part))
        && pre.is_none_or(|pre| identifiers(pre, true))
        && build.is_none_or(|build| identifiers(build, false))
}

fn tidy(value: &str, limit: usize) -> bool {
    text(value, limit, false).is_ok_and(|trimmed| trimmed == value)
}

fn capability_name(capability: Capability) -> String {
    match serde_json::to_value(capability) {
        Ok(Value::String(name)) => name,
        _ => unreachable!("a capability serialises as a string"),
    }
}

fn duplicates(list: &[Capability], field: &str, issues: &mut Issues) {
    let mut seen = BTreeSet::new();
    for (index, capability) in list.iter().enumerate() {
        if !seen.insert(*capability) {
            issues.add(
                ManifestRule::DuplicateCapability,
                format!("{field}[{index}]"),
                format!(
                    "\"{}\" is listed more than once.",
                    capability_name(*capability)
                ),
            );
        }
    }
}

/// Reads and validates a manifest. Every problem found is returned, each
/// with a stable code, the field and a sentence for the author.
pub fn validate_manifest(bytes: &[u8]) -> Result<PluginManifest, Vec<ManifestIssue>> {
    let mut issues = Issues(Vec::new());
    if bytes.len() > MAX_MANIFEST_BYTES {
        issues.add(
            ManifestRule::TooLarge,
            "",
            format!("A manifest is at most {} KiB.", MAX_MANIFEST_BYTES / 1024),
        );
        return Err(issues.0);
    }
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(error) => {
            issues.add(
                ManifestRule::MalformedJson,
                "",
                format!(
                    "The file is not valid JSON (line {}, column {}).",
                    error.line(),
                    error.column()
                ),
            );
            return Err(issues.0);
        }
    };
    shape(&value, "", MANIFEST_FIELDS, &mut issues);
    if !issues.0.is_empty() {
        return Err(issues.0);
    }
    let manifest: PluginManifest = match serde_json::from_value(value) {
        Ok(manifest) => manifest,
        Err(_) => {
            issues.add(
                ManifestRule::MalformedJson,
                "",
                "The manifest could not be read.",
            );
            return Err(issues.0);
        }
    };

    if !plugin_id(&manifest.id) {
        issues.add(
            ManifestRule::InvalidId,
            "id",
            "Use a reverse-domain identifier such as org.example.my-plugin: at least three lower-case segments of letters, digits and hyphens, each starting with a letter, 63 characters at most.",
        );
    } else if RESERVED_ID_PREFIXES.contains(&manifest.id.split('.').next().unwrap_or_default()) {
        issues.add(
            ManifestRule::ReservedId,
            "id",
            "Identifiers starting with \"builtin\" or \"infrawerf\" are reserved for InfraWerf itself.",
        );
    }
    if !tidy(&manifest.name, 40) {
        issues.add(
            ManifestRule::InvalidName,
            "name",
            "A name is 1 to 40 characters with no leading, trailing or control characters.",
        );
    }
    if !semantic_version(&manifest.version) {
        issues.add(
            ManifestRule::InvalidVersion,
            "version",
            "Use Semantic Versioning, for example 1.4.0 or 2.0.0-beta.1.",
        );
    }
    if manifest.api_version > PLUGIN_API_VERSION {
        issues.add(
            ManifestRule::NewerApiVersion,
            "api_version",
            format!(
                "This plugin needs plugin API v{}. This InfraWerf supports up to v{PLUGIN_API_VERSION}. Update InfraWerf to use it.",
                manifest.api_version
            ),
        );
    } else if manifest.api_version < MIN_PLUGIN_API_VERSION {
        issues.add(
            ManifestRule::InvalidApiVersion,
            "api_version",
            format!("Plugin API versions start at {MIN_PLUGIN_API_VERSION}."),
        );
    }
    for (name, value, limit) in [
        ("description", &manifest.description, 280),
        ("author", &manifest.author, 80),
        ("license", &manifest.license, 64),
    ] {
        if value.as_deref().is_some_and(|value| !tidy(value, limit)) {
            issues.add(
                ManifestRule::InvalidText,
                name,
                format!(
                    "Use 1 to {limit} characters on one line, with no leading or trailing spaces, or leave the field out."
                ),
            );
        }
    }
    if let Some(homepage) = manifest.homepage.as_deref() {
        let host = homepage.strip_prefix("https://").unwrap_or_default();
        if homepage.len() > 200
            || host.is_empty()
            || host.starts_with('/')
            || homepage
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || "\"<>\\^`{|}".contains(c))
        {
            issues.add(
                ManifestRule::InvalidUrl,
                "homepage",
                "Use an https:// address of at most 200 characters, or leave the field out.",
            );
        }
    }

    duplicates(&manifest.capabilities, "capabilities", &mut issues);
    for (index, capability) in manifest.capabilities.iter().enumerate() {
        if !plugin_may_request(*capability) {
            issues.add(
                ManifestRule::CapabilityNotAvailable,
                format!("capabilities[{index}]"),
                format!(
                    "No plugin may request \"{}\".",
                    capability_name(*capability)
                ),
            );
        }
    }

    if manifest.contributions.is_empty() {
        issues.add(
            ManifestRule::NoContributions,
            "contributions",
            "A plugin contributes at least one thing.",
        );
    }
    if manifest.contributions.len() > MAX_PLUGIN_CONTRIBUTIONS {
        issues.add(
            ManifestRule::TooManyContributions,
            "contributions",
            format!("A plugin contributes at most {MAX_PLUGIN_CONTRIBUTIONS} things."),
        );
    }
    let mut ids = BTreeSet::new();
    let mut used = BTreeSet::new();
    for (index, contribution) in manifest.contributions.iter().enumerate() {
        let at = format!("contributions[{index}]");
        if contribution.id.len() > 32 || !slug(&contribution.id) {
            issues.add(
                ManifestRule::InvalidContributionId,
                format!("{at}.id"),
                "Use lower-case letters, digits and hyphens, starting with a letter, 32 characters at most.",
            );
        } else if !ids.insert(contribution.id.as_str()) {
            issues.add(
                ManifestRule::DuplicateContribution,
                format!("{at}.id"),
                format!(
                    "\"{}\" is used by more than one contribution.",
                    contribution.id
                ),
            );
        }
        if !tidy(&contribution.title, 40) {
            issues.add(
                ManifestRule::InvalidTitle,
                format!("{at}.title"),
                "A title is 1 to 40 characters with no leading, trailing or control characters.",
            );
        }
        if contribution.capabilities.is_empty() {
            issues.add(
                ManifestRule::ContributionWithoutCapability,
                format!("{at}.capabilities"),
                "A contribution lists every capability it uses, and uses at least one.",
            );
        }
        duplicates(
            &contribution.capabilities,
            &format!("{at}.capabilities"),
            &mut issues,
        );
        for (position, capability) in contribution.capabilities.iter().enumerate() {
            used.insert(*capability);
            let field = format!("{at}.capabilities[{position}]");
            let name = capability_name(*capability);
            if !manifest.capabilities.contains(capability) {
                issues.add(
                    ManifestRule::CapabilityNotRequested,
                    field.clone(),
                    format!("\"{name}\" is not in the manifest's capabilities list."),
                );
            }
            if !contribution.point.permitted().contains(capability) {
                issues.add(
                    ManifestRule::CapabilityNotPermitted,
                    field,
                    format!("\"{name}\" cannot be used at this extension point."),
                );
            }
        }
    }
    for (index, capability) in manifest.capabilities.iter().enumerate() {
        if !used.contains(capability) {
            issues.add(
                ManifestRule::UnusedCapability,
                format!("capabilities[{index}]"),
                format!(
                    "\"{}\" is requested but no contribution uses it. Request only what is used.",
                    capability_name(*capability)
                ),
            );
        }
    }

    if issues.0.is_empty() {
        Ok(manifest)
    } else {
        Err(issues.0)
    }
}

impl PluginManifest {
    /// The registry entries this manifest describes, owned by the plugin.
    pub fn registry_contributions(&self) -> Vec<Contribution> {
        self.contributions
            .iter()
            .map(|entry| Contribution {
                id: format!("{}.{}", self.id, entry.id),
                point: entry.point,
                title: entry.title.clone(),
                owner: self.id.clone(),
                order: entry.order.unwrap_or(DEFAULT_PLUGIN_ORDER),
                capabilities: entry.capabilities.clone(),
            })
            .collect()
    }
}

/// The published JSON Schema, generated from the types above.
pub fn manifest_schema() -> String {
    let schema = schemars::schema_for!(PluginManifest);
    let mut text = serde_json::to_string_pretty(&schema).expect("a schema serialises");
    text.push('\n');
    text
}
