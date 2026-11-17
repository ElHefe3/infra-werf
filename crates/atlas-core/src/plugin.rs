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
use ts_rs::TS;

/// The plugin API this InfraWerf implements.
pub const PLUGIN_API_VERSION: u32 = 2;
/// The oldest plugin API this InfraWerf still accepts.
pub const MIN_PLUGIN_API_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "infrawerf-plugin.json";
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_PLUGIN_CONTRIBUTIONS: usize = 64;
/// First ID segments a plugin may not use.
pub const RESERVED_ID_PREFIXES: [&str; 2] = ["builtin", "infrawerf"];

/// The contents of `infrawerf-plugin.json` for plugin API v2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "InfraWerf plugin manifest (API v2)")]
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
    /// A read-only command this contribution runs, for a `collector` or a
    /// `health_check` (API v2). It needs `ssh_session` and
    /// `run_approved_command`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<PackCommand>,
    /// For a `collector` whose command reads a table: present its rows as
    /// workloads of a kind named by `title` (API v2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_kind: Option<PackWorkloadKind>,
    /// Required for a `remote_app`: the application to open (API v2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<PackApp>,
    /// For a `server_badge`: the pack observation the badge shows (API v2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge: Option<PackBadge>,
    /// For a `relationship_kind`: how the line is drawn (API v2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship: Option<PackRelationship>,
}

/// An application a pack names for a server's Apps list. It always listens
/// on the server's own localhost: there is no host field, and InfraWerf
/// opens it through the same loopback-only tunnel as any other application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PackApp {
    /// The port on the server's localhost, 1 to 65535.
    pub port: u16,
    /// The page to open first, starting with `/`; 512 characters at most.
    /// Defaults to `/`. Never a scheme or a host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Which pack observation a server badge shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PackBadge {
    /// The `id` of a contribution in this manifest whose command reads one
    /// value (a `line` or `json` extractor) and has no parameters.
    pub observation: String,
}

/// How a relationship of this kind is drawn. It is decoration only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PackRelationship {
    /// Defaults to `solid`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineStyle>,
    /// Defaults to `green`, the colour of a plain relationship.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<KindColor>,
}

/// The line of a relationship kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum LineStyle {
    /// An unbroken line.
    Solid,
    /// A line of dashes.
    Dashed,
    /// A line of dots.
    Dotted,
}

/// The colour of a relationship kind, from a fixed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum KindColor {
    /// The colour of a plain relationship.
    Green,
    /// Blue.
    Blue,
    /// Amber.
    Amber,
    /// Red.
    Red,
    /// Purple.
    Purple,
    /// Grey.
    Grey,
}

/// Which columns of a collector's table name a workload and give its state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PackWorkloadKind {
    /// The column that holds each workload's name.
    pub name_column: String,
    /// The column that holds its state, if the table has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_column: Option<String>,
}

/// One read-only command a pack contribution runs, and how its output is
/// read (plugin API v2). It is an argument vector, never a shell string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PackCommand {
    /// The program, then its arguments: 1 to 16 entries. An entry holds
    /// only letters, digits and `_ . , : / = + @ % -`, or is exactly
    /// `{name}` for a declared parameter. The program is a plain name or an
    /// absolute path and is never a shell, an interpreter or `sudo`.
    pub argv: Vec<String>,
    /// Values filled in from what InfraWerf discovered on the server, at
    /// most 4. Each must be used by `argv`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<PackParameter>,
    /// How the output is read.
    pub extract: Extractor,
    /// For a health check: the values that mean healthy, 1 to 8. Any other
    /// value means unhealthy; no readable value means unknown. Required at
    /// `health_check`, not allowed at `collector`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healthy: Option<Vec<String>>,
}

/// A value InfraWerf fills in. The owner picks it from what an authenticated
/// session discovered; a pack cannot supply it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PackParameter {
    /// Lower-case letters, digits and underscores, starting with a letter,
    /// 32 characters at most. Written in `argv` as `{name}`.
    pub name: String,
    pub source: ParameterSource,
}

/// Where a parameter's value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ParameterSource {
    /// The name of a systemd service found in the server's inventory.
    Service,
    /// The name of a container found in the server's inventory.
    Container,
}

/// How a command's output becomes a value or a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Extractor {
    /// One line of the output as a single value.
    Line {
        /// Which line, counting from 1. Defaults to 1; 1000 at most.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u16>,
    },
    /// Each line as a row of named columns. At most 100 rows are kept.
    Table {
        /// 1 to 12 column names: lower-case letters, digits and
        /// underscores, starting with a letter. The last column takes the
        /// rest of the line.
        columns: Vec<String>,
        /// Lines to skip before the first row, 100 at most. Defaults to 0.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skip_lines: Option<u16>,
        /// What separates columns. Defaults to `whitespace`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        separator: Option<Separator>,
    },
    /// One text, number or true/false value from JSON output.
    Json {
        /// Dotted names with optional `[index]`, for example
        /// `pool.disks[0].state`; 200 characters at most.
        path: String,
    },
}

/// What separates the columns of a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Separator {
    /// Any run of spaces or tabs.
    Whitespace,
    /// One tab character.
    Tab,
    /// One comma.
    Comma,
}

pub const MAX_PACK_ARGUMENTS: usize = 16;
pub const MAX_PACK_PARAMETERS: usize = 4;
/// The API version that introduced pack commands.
pub const PACK_API_VERSION: u32 = 2;
/// Programs a pack may not name, by file name: shells, interpreters and
/// wrappers that run other programs, ways to become another user, network
/// clients, and the common programs whose purpose is to change a system.
/// This catches mistakes and the obvious abuses. It cannot prove that a
/// program only reads, which is why the owner is shown the exact command.
pub const FORBIDDEN_PROGRAMS: &[&str] = &[
    "sh",
    "bash",
    "dash",
    "zsh",
    "ksh",
    "csh",
    "tcsh",
    "fish",
    "ash",
    "busybox",
    "env",
    "sudo",
    "su",
    "doas",
    "pkexec",
    "runuser",
    "perl",
    "ruby",
    "node",
    "php",
    "lua",
    "awk",
    "gawk",
    "mawk",
    "sed",
    "xargs",
    "find",
    "eval",
    "exec",
    "nohup",
    "timeout",
    "nice",
    "ionice",
    "strace",
    "watch",
    "ssh",
    "scp",
    "sftp",
    "rsync",
    "curl",
    "wget",
    "nc",
    "ncat",
    "socat",
    "telnet",
    "rm",
    "mv",
    "cp",
    "dd",
    "tee",
    "chmod",
    "chown",
    "kill",
    "pkill",
    "killall",
    "reboot",
    "shutdown",
    "poweroff",
    "halt",
    "mkfs",
    "mount",
    "umount",
    "truncate",
    "ln",
    "mkdir",
    "rmdir",
    "touch",
    "install",
    "tar",
    "systemd-run",
    "at",
    "crontab",
];

fn forbidden_program(program: &str) -> bool {
    let name = program.rsplit('/').next().unwrap_or(program);
    FORBIDDEN_PROGRAMS.contains(&name) || name.starts_with("python")
}

fn field_name(name: &str) -> bool {
    name.len() <= 32
        && name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn json_path_ok(path: &str) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    !path.is_empty()
        && path.len() <= 200
        && segments.len() <= 16
        && segments.iter().all(|segment| {
            let (name, indexes) = match segment.find('[') {
                Some(at) => (&segment[..at], &segment[at..]),
                None => (*segment, ""),
            };
            let named = name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
            let indexed = indexes.is_empty()
                || (indexes.ends_with(']')
                    && indexes.split_terminator(']').all(|index| {
                        index.strip_prefix('[').is_some_and(|digits| {
                            !digits.is_empty()
                                && digits.len() <= 4
                                && digits.bytes().all(|b| b.is_ascii_digit())
                        })
                    }));
            named && indexed && !(name.is_empty() && indexes.is_empty())
        })
}

/// Checks one pack command. `at` is the field path of the command.
fn check_command(
    command: &PackCommand,
    contribution: &ManifestContribution,
    api_version: u32,
    at: &str,
    issues: &mut Issues,
) {
    use crate::pack::{placeholder, safe_argument};
    let health = contribution.point == ExtensionPoint::HealthCheck;
    if api_version < PACK_API_VERSION {
        issues.add(
            ManifestRule::CommandNotSupported,
            at,
            format!("Commands need \"api_version\": {PACK_API_VERSION} or newer."),
        );
    }
    if !health && contribution.point != ExtensionPoint::Collector {
        issues.add(
            ManifestRule::CommandNotSupported,
            at,
            "Only a collector or a health_check contribution may have a command.",
        );
    }
    for needed in [Capability::SshSession, Capability::RunApprovedCommand] {
        if !contribution.capabilities.contains(&needed) {
            issues.add(
                ManifestRule::CommandNeedsCapability,
                at,
                format!(
                    "A contribution with a command lists \"{}\" in its capabilities.",
                    capability_name(needed)
                ),
            );
        }
    }
    if command.argv.is_empty() || command.argv.len() > MAX_PACK_ARGUMENTS {
        issues.add(
            ManifestRule::UnsafeArgument,
            format!("{at}.argv"),
            format!("A command is a program and its arguments: 1 to {MAX_PACK_ARGUMENTS} entries."),
        );
    }
    let mut names = BTreeSet::new();
    if command.parameters.len() > MAX_PACK_PARAMETERS {
        issues.add(
            ManifestRule::InvalidParameter,
            format!("{at}.parameters"),
            format!("A command has at most {MAX_PACK_PARAMETERS} parameters."),
        );
    }
    for (index, parameter) in command.parameters.iter().enumerate() {
        if !field_name(&parameter.name) || !names.insert(parameter.name.as_str()) {
            issues.add(
                ManifestRule::InvalidParameter,
                format!("{at}.parameters[{index}].name"),
                "Use a name of lower-case letters, digits and underscores, starting with a letter, 32 characters at most, and use it once.",
            );
        }
    }
    let mut used = BTreeSet::new();
    for (index, argument) in command.argv.iter().enumerate() {
        let field = format!("{at}.argv[{index}]");
        if let Some(name) = placeholder(argument) {
            if index == 0 {
                issues.add(
                    ManifestRule::UnsafeArgument,
                    field,
                    "The program cannot be a parameter.",
                );
            } else if names.contains(name) {
                used.insert(name);
            } else {
                issues.add(
                    ManifestRule::UndeclaredParameter,
                    field,
                    format!("\"{name}\" is not in this command's parameters list."),
                );
            }
            continue;
        }
        if !safe_argument(argument) {
            issues.add(
                ManifestRule::UnsafeArgument,
                field,
                "An argument is 1 to 200 characters of letters, digits and _ . , : / = + @ % - only. Spaces, quotes, pipes, redirects, $ and other shell characters are not allowed: a command is not run by a shell.",
            );
        } else if index == 0 {
            let path = argument.contains('/');
            if argument.starts_with('-') || (path && !argument.starts_with('/')) {
                issues.add(
                    ManifestRule::UnsafeArgument,
                    field,
                    "The program is a plain name such as df, or an absolute path such as /usr/bin/df.",
                );
            } else if forbidden_program(argument) {
                issues.add(
                    ManifestRule::ForbiddenProgram,
                    field,
                    format!("A pack may not run \"{argument}\": no shells, interpreters, sudo, network clients or programs that change a server."),
                );
            }
        }
    }
    for (index, parameter) in command.parameters.iter().enumerate() {
        if names.contains(parameter.name.as_str()) && !used.contains(parameter.name.as_str()) {
            issues.add(
                ManifestRule::UnusedParameter,
                format!("{at}.parameters[{index}]"),
                format!("\"{}\" is declared but argv never uses it.", parameter.name),
            );
        }
    }
    let extract = format!("{at}.extract");
    match &command.extract {
        Extractor::Line { line } => {
            if line.is_some_and(|line| line == 0 || line > 1000) {
                issues.add(
                    ManifestRule::InvalidExtractor,
                    format!("{extract}.line"),
                    "Lines count from 1, up to 1000.",
                );
            }
        }
        Extractor::Table {
            columns,
            skip_lines,
            ..
        } => {
            let mut seen = BTreeSet::new();
            if columns.is_empty()
                || columns.len() > 12
                || !columns
                    .iter()
                    .all(|column| field_name(column) && seen.insert(column.as_str()))
            {
                issues.add(
                    ManifestRule::InvalidExtractor,
                    format!("{extract}.columns"),
                    "Give 1 to 12 different column names of lower-case letters, digits and underscores, each starting with a letter.",
                );
            }
            if skip_lines.is_some_and(|skip| skip > 100) {
                issues.add(
                    ManifestRule::InvalidExtractor,
                    format!("{extract}.skip_lines"),
                    "At most 100 lines can be skipped.",
                );
            }
        }
        Extractor::Json { path } => {
            if !json_path_ok(path) {
                issues.add(
                    ManifestRule::InvalidExtractor,
                    format!("{extract}.path"),
                    "Use dotted names with optional [index], for example pool.disks[0].state.",
                );
            }
        }
    }
    let healthy = format!("{at}.healthy");
    match (&command.healthy, health) {
        (None, true) => issues.add(
            ManifestRule::InvalidHealthRule,
            healthy,
            "A health check lists the values that mean healthy.",
        ),
        (Some(_), false) => issues.add(
            ManifestRule::InvalidHealthRule,
            healthy,
            "Only a health_check contribution has \"healthy\" values.",
        ),
        (Some(values), true) => {
            if matches!(command.extract, Extractor::Table { .. }) {
                issues.add(
                    ManifestRule::InvalidHealthRule,
                    healthy,
                    "A health check reads one value: use a line or json extractor.",
                );
            } else if values.is_empty()
                || values.len() > 8
                || !values.iter().all(|value| tidy(value, 64))
            {
                issues.add(
                    ManifestRule::InvalidHealthRule,
                    healthy,
                    "List 1 to 8 values of 1 to 64 characters each.",
                );
            }
        }
        (None, false) => {}
    }
}

/// Checks what API v2 added besides commands: applications, badges,
/// relationship kinds and workload kinds. `at` is the contribution's path.
fn check_additions(
    manifest: &PluginManifest,
    contribution: &ManifestContribution,
    at: &str,
    issues: &mut Issues,
) {
    use ExtensionPoint::*;
    let v2 = manifest.api_version >= PACK_API_VERSION;
    let needs_v2 = format!("needs \"api_version\": {PACK_API_VERSION} or newer");
    if !v2 && matches!(contribution.point, RemoteApp | RelationshipKind) {
        issues.add(
            ManifestRule::UnknownPoint,
            format!("{at}.point"),
            format!("This extension point {needs_v2}."),
        );
    }

    let field = format!("{at}.app");
    match (&contribution.app, contribution.point == RemoteApp) {
        (None, true) => issues.add(
            ManifestRule::InvalidApp,
            field,
            "A remote_app contribution has an \"app\" that gives the port to open.",
        ),
        (Some(_), false) => issues.add(
            ManifestRule::InvalidApp,
            field,
            "Only a remote_app contribution has an \"app\".",
        ),
        (Some(app), true) => {
            for needed in [Capability::SshSession, Capability::LoopbackTunnel] {
                if !contribution.capabilities.contains(&needed) {
                    issues.add(
                        ManifestRule::InvalidApp,
                        format!("{at}.capabilities"),
                        format!(
                            "A remote_app lists \"{}\" in its capabilities: it opens through your SSH access.",
                            capability_name(needed)
                        ),
                    );
                }
            }
            if app.port == 0 {
                issues.add(
                    ManifestRule::InvalidApp,
                    format!("{field}.port"),
                    "A port is a whole number from 1 to 65535.",
                );
            }
            if app
                .path
                .as_deref()
                .is_some_and(|path| !crate::tunnel::valid_path(path))
            {
                issues.add(
                    ManifestRule::InvalidApp,
                    format!("{field}.path"),
                    "A path starts with one / and is at most 512 characters, for example /login. It is never a full address: an application is always opened on the server's own localhost.",
                );
            }
        }
        (None, false) => {}
    }

    if let Some(badge) = &contribution.badge {
        let field = format!("{at}.badge");
        if contribution.point != ServerBadge {
            issues.add(
                ManifestRule::InvalidBadge,
                field,
                "Only a server_badge contribution has a \"badge\".",
            );
        } else if !v2 {
            issues.add(
                ManifestRule::InvalidBadge,
                field,
                format!("A badge {needs_v2}."),
            );
        } else {
            let single_value = manifest
                .contributions
                .iter()
                .find(|other| other.id == badge.observation)
                .and_then(|other| other.command.as_ref())
                .is_some_and(|command| {
                    command.parameters.is_empty()
                        && !matches!(command.extract, Extractor::Table { .. })
                });
            if !single_value {
                issues.add(
                    ManifestRule::InvalidBadge,
                    format!("{field}.observation"),
                    format!(
                        "\"{}\" must be the id of a contribution in this manifest whose command reads one value (a line or json extractor) and has no parameters.",
                        badge.observation
                    ),
                );
            }
        }
    }

    if contribution.relationship.is_some() {
        let field = format!("{at}.relationship");
        if contribution.point != RelationshipKind {
            issues.add(
                ManifestRule::InvalidRelationshipKind,
                field,
                "Only a relationship_kind contribution has a \"relationship\".",
            );
        }
    }

    if let Some(kind) = &contribution.workload_kind {
        let field = format!("{at}.workload_kind");
        let columns = match (&contribution.command, contribution.point) {
            (Some(command), Collector) => match &command.extract {
                Extractor::Table { columns, .. } => Some(columns),
                _ => None,
            },
            _ => None,
        };
        match columns {
            None => issues.add(
                ManifestRule::InvalidWorkloadKind,
                field,
                "Only a collector whose command has a table extractor can describe a workload kind.",
            ),
            Some(columns) => {
                if !columns.contains(&kind.name_column) {
                    issues.add(
                        ManifestRule::InvalidWorkloadKind,
                        format!("{field}.name_column"),
                        format!(
                            "\"{}\" is not one of the table's columns.",
                            kind.name_column
                        ),
                    );
                }
                if kind.state_column.as_ref().is_some_and(|state| {
                    !columns.contains(state) || *state == kind.name_column
                }) {
                    issues.add(
                        ManifestRule::InvalidWorkloadKind,
                        format!("{field}.state_column"),
                        "The state column is another one of the table's columns, or is left out.",
                    );
                }
            }
        }
    }
}

impl std::fmt::Display for PackCommand {
    /// The command as the owner reads it: the arguments separated by spaces.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.argv.join(" "))
    }
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
    /// A command has an unknown field, a missing field or a wrong type.
    InvalidCommand,
    /// A command where none is possible: the wrong extension point, or a
    /// manifest written for API v1.
    CommandNotSupported,
    /// A contribution with a command does not list the two capabilities a
    /// command uses.
    CommandNeedsCapability,
    /// An argument holds a character outside the safe set, or the command
    /// has no program or too many arguments.
    UnsafeArgument,
    /// The program is a shell, an interpreter, sudo or another program a
    /// pack may not run.
    ForbiddenProgram,
    /// `argv` uses a `{name}` that the command does not declare.
    UndeclaredParameter,
    /// A declared parameter that `argv` never uses.
    UnusedParameter,
    /// A parameter name is malformed or repeated, or there are too many.
    InvalidParameter,
    /// A line number, column list or JSON path is malformed.
    InvalidExtractor,
    /// The healthy values are missing, misplaced or malformed.
    InvalidHealthRule,
    /// An application is missing, misplaced or malformed: it gives a port
    /// and a path on the server's own localhost and nothing else.
    InvalidApp,
    /// A badge is misplaced or malformed, or does not name a contribution
    /// in the same manifest that reads one value without parameters.
    InvalidBadge,
    /// A relationship style is misplaced or malformed.
    InvalidRelationshipKind,
    /// A workload kind is misplaced or malformed, or names a column the
    /// table does not have.
    InvalidWorkloadKind,
}

impl ManifestRule {
    pub const ALL: [ManifestRule; 40] = [
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
        Self::InvalidCommand,
        Self::CommandNotSupported,
        Self::CommandNeedsCapability,
        Self::UnsafeArgument,
        Self::ForbiddenProgram,
        Self::UndeclaredParameter,
        Self::UnusedParameter,
        Self::InvalidParameter,
        Self::InvalidExtractor,
        Self::InvalidHealthRule,
        Self::InvalidApp,
        Self::InvalidBadge,
        Self::InvalidRelationshipKind,
        Self::InvalidWorkloadKind,
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
    Command,
    WorkloadKind,
    App,
    Badge,
    Relationship,
}
impl Kind {
    fn expects(self) -> &'static str {
        match self {
            Kind::Text | Kind::Point => "a string",
            Kind::Integer => "a whole number",
            Kind::Capabilities => "a list of capability names",
            Kind::Contributions => "a list of contributions",
            Kind::Command => "a command object",
            Kind::WorkloadKind => "a workload kind object",
            Kind::App => "an application object",
            Kind::Badge => "a badge object",
            Kind::Relationship => "a relationship style object",
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
    ("command", Kind::Command, false),
    ("workload_kind", Kind::WorkloadKind, false),
    ("app", Kind::App, false),
    ("badge", Kind::Badge, false),
    ("relationship", Kind::Relationship, false),
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
            Kind::Command => {
                if !found.is_object() {
                    wrong(issues, field);
                } else if let Err(error) = serde_json::from_value::<PackCommand>(found.clone()) {
                    // serde names the unknown or missing field or the wrong type.
                    let reason = error.to_string();
                    let reason = reason.split(" at line ").next().unwrap_or_default();
                    issues.add(
                        ManifestRule::InvalidCommand,
                        field,
                        format!("The command is not written as plugin API v{PLUGIN_API_VERSION} describes: {reason}."),
                    );
                }
            }
            Kind::WorkloadKind | Kind::App | Kind::Badge | Kind::Relationship => {
                let (rule, read) = match kind {
                    Kind::WorkloadKind => (
                        ManifestRule::InvalidWorkloadKind,
                        serde_json::from_value::<PackWorkloadKind>(found.clone()).err(),
                    ),
                    Kind::App => (
                        ManifestRule::InvalidApp,
                        serde_json::from_value::<PackApp>(found.clone()).err(),
                    ),
                    Kind::Badge => (
                        ManifestRule::InvalidBadge,
                        serde_json::from_value::<PackBadge>(found.clone()).err(),
                    ),
                    _ => (
                        ManifestRule::InvalidRelationshipKind,
                        serde_json::from_value::<PackRelationship>(found.clone()).err(),
                    ),
                };
                if !found.is_object() {
                    wrong(issues, field);
                } else if let Some(error) = read {
                    let reason = error.to_string();
                    let reason = reason.split(" at line ").next().unwrap_or_default();
                    issues.add(
                        rule,
                        field,
                        format!("This is not written as plugin API v{PLUGIN_API_VERSION} describes: {reason}."),
                    );
                }
            }
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
            // API v1 allowed a health check only an unauthenticated probe.
            let v1_health = manifest.api_version < PACK_API_VERSION
                && contribution.point == ExtensionPoint::HealthCheck
                && *capability != Capability::NetworkProbe;
            if v1_health || !contribution.point.permitted().contains(capability) {
                issues.add(
                    ManifestRule::CapabilityNotPermitted,
                    field,
                    format!("\"{name}\" cannot be used at this extension point."),
                );
            }
        }
        if let Some(command) = &contribution.command {
            check_command(
                command,
                contribution,
                manifest.api_version,
                &format!("{at}.command"),
                &mut issues,
            );
        }
        check_additions(&manifest, contribution, &at, &mut issues);
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
