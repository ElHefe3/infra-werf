//! Declarative pack commands (S12/F21/T043).
//!
//! A pack is plugin content made only of data: a contribution at the
//! `collector` or `health_check` point may carry one read-only command and
//! say how to read its output. This module turns such a command into the
//! exact argument vector to run and turns the bounded output into a small,
//! typed result. It runs nothing itself: the command goes through the same
//! authenticated, host-pinned session and operation broker as a built-in
//! collector (`FileService::run_pack_command`), after the owner's grants
//! were checked (`Store::plugin_begin_run`).
//!
//! Raw output never leaves the core. Only the extracted values do, each
//! stripped of control characters and cut to a fixed length.
use crate::plugin::{Extractor, PackCommand, ParameterSource, Separator};
use crate::workloads::{WorkloadIdentity, WorkloadKind};
use crate::{AtlasError, CONTRACT_VERSION, ErrorCode, ServerId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::{Config, TS};

/// Rows kept from a table; further rows are dropped and `truncated` is set.
pub const MAX_PACK_ROWS: usize = 100;
/// Characters kept from one extracted value.
pub const MAX_PACK_VALUE_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PackOutcome {
    /// The command ran and its output was read.
    Ok,
    /// The command ended with a failure status.
    CommandFailed,
    /// The command ran but the output did not hold what the pack describes.
    Unreadable,
    /// The output was larger than the limit for a built-in collector.
    OutputLimit,
    /// The command ran longer than the limit for a built-in collector.
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PackHealth {
    Healthy,
    Unhealthy,
    /// The check could not be read, so nothing is claimed either way.
    Unknown,
}

/// What was read from one run of a pack command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PackResult {
    pub outcome: PackOutcome,
    /// A single value, from a `line` or `json` extractor.
    pub value: Option<String>,
    /// Column names, from a `table` extractor.
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// More rows existed than are kept.
    pub truncated: bool,
    /// Set for a health check only.
    pub health: Option<PackHealth>,
}

impl PackResult {
    /// A result for a run that produced nothing readable.
    pub fn failed(command: &PackCommand, outcome: PackOutcome) -> Self {
        Self {
            outcome,
            value: None,
            columns: Vec::new(),
            rows: Vec::new(),
            truncated: false,
            health: command.healthy.as_ref().map(|_| PackHealth::Unknown),
        }
    }
}

/// One observation from a pack command, as shown for a server. It is held
/// in memory for this run of InfraWerf only and is never written to disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PackObservation {
    /// When InfraWerf read the result, on this PC's clock.
    pub observed_at: String,
    /// False once the observation is older than the staleness limit.
    pub fresh: bool,
    /// Which plugin contribution produced it, as `plugin-id.contribution`.
    pub source: String,
    /// Binding of the authenticated SSH profile and host key it came through.
    pub source_binding: String,
    /// Parameter name to the discovered workload it was run for.
    pub parameters: BTreeMap<String, String>,
    pub result: PackResult,
}

/// A parameter of a pack command with the discovered values it may take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PackParameterView {
    pub name: String,
    pub source: ParameterSource,
    /// Workloads from the server's saved inventory that may be chosen.
    pub choices: Vec<PackChoice>,
}

/// One discovered workload a parameter may be set to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PackChoice {
    /// The workload's ID; this is what a run is given, never a name.
    pub id: String,
    pub name: String,
}

/// The discovered workloads a parameter may be set to: supported ones of
/// the right kind whose name is a safe argument. A `service` is a
/// system-scope systemd unit.
fn candidates(
    source: ParameterSource,
    workloads: &[WorkloadIdentity],
) -> impl Iterator<Item = &WorkloadIdentity> {
    workloads.iter().filter(move |w| {
        let kind = match source {
            ParameterSource::Service => w.kind == WorkloadKind::Service && w.scope == "system",
            ParameterSource::Container => w.kind == WorkloadKind::Container,
        };
        kind && w.supported && safe_argument(&w.name) && !w.name.starts_with('-')
    })
}

/// The workloads a parameter may take, from saved inventory. A run checks
/// the chosen ID again against what its own authenticated session found.
pub fn choices(source: ParameterSource, workloads: &[WorkloadIdentity]) -> Vec<PackChoice> {
    candidates(source, workloads)
        .map(|w| PackChoice {
            id: w.id.clone(),
            name: w.name.clone(),
        })
        .collect()
}

/// One pack command available on a server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PackCommandView {
    pub plugin_id: String,
    pub plugin_name: String,
    pub contribution_id: String,
    pub title: String,
    pub health_check: bool,
    /// The exact command, with `{name}` where a parameter goes.
    pub argv: Vec<String>,
    pub parameters: Vec<PackParameterView>,
    pub observation: Option<PackObservation>,
}

/// Every pack command the owner allowed on one server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PackServerState {
    pub contract_version: u32,
    pub server_id: ServerId,
    pub commands: Vec<PackCommandView>,
}

impl PackServerState {
    pub fn new(server_id: ServerId, commands: Vec<PackCommandView>) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            server_id,
            commands,
        }
    }
}

fn denied() -> AtlasError {
    AtlasError::new(ErrorCode::Denied)
}

/// Characters an argument may hold, whether written by the pack or filled in
/// from discovery. None of them means anything to a shell.
pub fn safe_argument(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.,:/=+@%-".contains(&b))
}

/// The parameter an argument stands for, when it is exactly `{name}`.
pub fn placeholder(argument: &str) -> Option<&str> {
    argument.strip_prefix('{')?.strip_suffix('}')
}

/// The argument vector to run. Each parameter value is the ID of a workload
/// that this authenticated session discovered; anything else is refused.
/// Returns the vector and, per parameter, the workload's name for display.
pub fn resolve(
    command: &PackCommand,
    values: &BTreeMap<String, String>,
    workloads: &[WorkloadIdentity],
) -> Result<(Vec<String>, BTreeMap<String, String>), AtlasError> {
    if values.len() != command.parameters.len() {
        return Err(denied());
    }
    let mut names = BTreeMap::new();
    for parameter in &command.parameters {
        let id = values.get(&parameter.name).ok_or_else(denied)?;
        let found = candidates(parameter.source, workloads)
            .find(|w| w.id == *id)
            .ok_or_else(denied)?;
        names.insert(parameter.name.clone(), found.name.clone());
    }
    let mut argv = Vec::with_capacity(command.argv.len());
    for argument in &command.argv {
        match placeholder(argument) {
            Some(name) => argv.push(names.get(name).ok_or_else(denied)?.clone()),
            None => argv.push(argument.clone()),
        }
    }
    Ok((argv, names))
}

fn clean(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_PACK_VALUE_CHARS)
        .collect()
}

fn json_path<'a>(mut value: &'a Value, path: &str) -> Option<&'a Value> {
    for segment in path.split('.') {
        let (name, indexes) = match segment.find('[') {
            Some(at) => (&segment[..at], &segment[at..]),
            None => (segment, ""),
        };
        if !name.is_empty() {
            value = value.as_object()?.get(name)?;
        }
        for index in indexes.split_terminator(']') {
            let index: usize = index.strip_prefix('[')?.parse().ok()?;
            value = value.as_array()?.get(index)?;
        }
    }
    Some(value)
}

/// Read a finished command's output as the pack describes it.
pub fn interpret(command: &PackCommand, success: bool, stdout: &str) -> PackResult {
    // A collector needs the command to succeed. A health check is judged by
    // what the command printed, because status commands such as
    // `systemctl is-active` report "inactive" with a failure status.
    if !success && command.healthy.is_none() {
        return PackResult::failed(command, PackOutcome::CommandFailed);
    }
    let mut result = PackResult::failed(command, PackOutcome::Ok);
    match &command.extract {
        Extractor::Line { line } => {
            let wanted = usize::from(line.unwrap_or(1).max(1)) - 1;
            result.value = stdout
                .lines()
                .nth(wanted)
                .map(clean)
                .filter(|v| !v.is_empty());
        }
        Extractor::Json { path } => {
            result.value = serde_json::from_str::<Value>(stdout)
                .ok()
                .as_ref()
                .and_then(|value| json_path(value, path))
                .and_then(|value| match value {
                    Value::String(text) => Some(clean(text)),
                    Value::Number(number) => Some(number.to_string()),
                    Value::Bool(flag) => Some(flag.to_string()),
                    _ => None,
                })
                .filter(|v| !v.is_empty());
        }
        Extractor::Table {
            columns,
            skip_lines,
            separator,
        } => {
            let count = columns.len();
            result.columns = columns.clone();
            for line in stdout
                .lines()
                .skip(usize::from(skip_lines.unwrap_or(0)))
                .filter(|line| !line.trim().is_empty())
            {
                let cells: Vec<String> = match separator.unwrap_or(Separator::Whitespace) {
                    Separator::Tab => line.splitn(count, '\t').map(clean).collect(),
                    Separator::Comma => line.splitn(count, ',').map(clean).collect(),
                    Separator::Whitespace => {
                        // The last column takes the rest of the line.
                        let mut rest = line.trim();
                        let mut cells = Vec::new();
                        while cells.len() + 1 < count {
                            match rest.split_once(char::is_whitespace) {
                                Some((cell, tail)) => {
                                    cells.push(clean(cell));
                                    rest = tail.trim_start();
                                }
                                None => break,
                            }
                        }
                        cells.push(clean(rest));
                        cells
                    }
                };
                // A line with too few cells is not a row of this table.
                if cells.len() != count {
                    result.columns.clear();
                    result.rows.clear();
                    result.outcome = PackOutcome::Unreadable;
                    return result;
                }
                if result.rows.len() == MAX_PACK_ROWS {
                    result.truncated = true;
                    break;
                }
                result.rows.push(cells);
            }
        }
    }
    let single = !matches!(command.extract, Extractor::Table { .. });
    if single && result.value.is_none() {
        result.outcome = if success {
            PackOutcome::Unreadable
        } else {
            PackOutcome::CommandFailed
        };
    }
    if let Some(healthy) = &command.healthy {
        result.health = Some(match &result.value {
            Some(value) if healthy.contains(value) => PackHealth::Healthy,
            Some(_) => PackHealth::Unhealthy,
            None => PackHealth::Unknown,
        });
    }
    result
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        ParameterSource::decl(config),
        Separator::decl(config),
        Extractor::decl(config),
        crate::plugin::PackParameter::decl(config),
        PackCommand::decl(config),
        PackOutcome::decl(config),
        PackHealth::decl(config),
        PackResult::decl(config),
        PackObservation::decl(config),
        PackChoice::decl(config),
        PackParameterView::decl(config),
        PackCommandView::decl(config),
        PackServerState::decl(config),
    ]
    .into_iter()
    .map(|d| format!("export {d}\n"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(extract: Extractor, healthy: Option<&[&str]>) -> PackCommand {
        PackCommand {
            argv: vec!["systemctl".into(), "is-active".into(), "{unit}".into()],
            parameters: vec![crate::plugin::PackParameter {
                name: "unit".into(),
                source: ParameterSource::Service,
            }],
            extract,
            healthy: healthy.map(|v| v.iter().map(|s| (*s).to_owned()).collect()),
        }
    }

    #[test]
    fn a_line_is_read_and_judged_against_the_healthy_values() {
        let check = command(Extractor::Line { line: None }, Some(&["active"]));
        let healthy = interpret(&check, true, "active\n");
        assert_eq!(healthy.value.as_deref(), Some("active"));
        assert_eq!(healthy.health, Some(PackHealth::Healthy));
        assert_eq!(
            interpret(&check, true, "failed\n").health,
            Some(PackHealth::Unhealthy)
        );
        // No output and a failed command claim nothing either way.
        let empty = interpret(&check, true, "");
        assert_eq!(empty.outcome, PackOutcome::Unreadable);
        assert_eq!(empty.health, Some(PackHealth::Unknown));
        // A status command that fails but says why is judged by its word;
        // one that fails silently claims nothing.
        let inactive = interpret(&check, false, "inactive\n");
        assert_eq!(inactive.outcome, PackOutcome::Ok);
        assert_eq!(inactive.health, Some(PackHealth::Unhealthy));
        let silent = interpret(&check, false, "");
        assert_eq!(silent.outcome, PackOutcome::CommandFailed);
        assert_eq!(silent.health, Some(PackHealth::Unknown));
        // A collector's failed command yields nothing at all.
        let reader = command(Extractor::Line { line: None }, None);
        let failed = interpret(&reader, false, "partial\n");
        assert_eq!(failed.outcome, PackOutcome::CommandFailed);
        assert_eq!(failed.value, None);
    }

    fn workload(id: &str, kind: WorkloadKind, scope: &str, name: &str) -> WorkloadIdentity {
        WorkloadIdentity {
            id: id.into(),
            kind,
            scope: scope.into(),
            runtime: String::new(),
            name: name.into(),
            reported_state: String::new(),
            reported_detail: String::new(),
            supported: true,
        }
    }

    #[test]
    fn a_parameter_takes_only_a_workload_this_session_discovered() {
        let check = command(Extractor::Line { line: None }, Some(&["active"]));
        let found = [
            workload("wl1_a", WorkloadKind::Service, "system", "nginx.service"),
            workload("wl1_b", WorkloadKind::Service, "user", "sync.service"),
            workload("wl1_c", WorkloadKind::Container, "system", "db"),
            workload("wl1_d", WorkloadKind::Service, "system", "-rf"),
            workload("wl1_e", WorkloadKind::Service, "system", "a;reboot"),
        ];
        let with = |id: &str| BTreeMap::from([("unit".to_owned(), id.to_owned())]);
        let (argv, names) = resolve(&check, &with("wl1_a"), &found).unwrap();
        assert_eq!(argv, ["systemctl", "is-active", "nginx.service"]);
        assert_eq!(names["unit"], "nginx.service");
        assert_eq!(choices(ParameterSource::Service, &found).len(), 1);
        // A name instead of an ID, an unknown ID, the wrong kind or scope,
        // and names that are not safe arguments are all refused.
        for id in [
            "nginx.service",
            "wl1_zz",
            "wl1_b",
            "wl1_c",
            "wl1_d",
            "wl1_e",
        ] {
            assert!(resolve(&check, &with(id), &found).is_err(), "{id}");
        }
        // Missing, extra and misnamed values are refused.
        assert!(resolve(&check, &BTreeMap::new(), &found).is_err());
        let extra = BTreeMap::from([
            ("unit".to_owned(), "wl1_a".to_owned()),
            ("other".to_owned(), "wl1_a".to_owned()),
        ]);
        assert!(resolve(&check, &extra, &found).is_err());
        let misnamed = BTreeMap::from([("other".to_owned(), "wl1_a".to_owned())]);
        assert!(resolve(&check, &misnamed, &found).is_err());
    }

    #[test]
    fn values_lose_control_characters_and_are_cut_to_length() {
        let reader = command(Extractor::Line { line: Some(2) }, None);
        let long = "x".repeat(500);
        let result = interpret(&reader, true, &format!("first\n\u{1b}[31m{long}\n"));
        let value = result.value.unwrap();
        assert_eq!(value.chars().count(), MAX_PACK_VALUE_CHARS);
        assert!(!value.chars().any(char::is_control));
    }

    #[test]
    fn a_table_keeps_named_columns_bounded_rows_and_refuses_short_lines() {
        let table = command(
            Extractor::Table {
                columns: vec!["filesystem".into(), "used".into(), "mounted_on".into()],
                skip_lines: Some(1),
                separator: None,
            },
            None,
        );
        let result = interpret(
            &table,
            true,
            "Filesystem Use% Mounted on\n/dev/sda1  41%  /\n/dev/sdb1  87%  /srv/media library\n",
        );
        assert_eq!(result.outcome, PackOutcome::Ok);
        assert_eq!(result.rows[0], ["/dev/sda1", "41%", "/"]);
        assert_eq!(result.rows[1], ["/dev/sdb1", "87%", "/srv/media library"]);
        assert_eq!(
            interpret(&table, true, "header\nonly-one-cell\n").outcome,
            PackOutcome::Unreadable
        );
        let many: String = (0..150).map(|n| format!("d{n} 1% /m{n}\n")).collect();
        let cut = interpret(&table, true, &format!("header\n{many}"));
        assert_eq!(cut.rows.len(), MAX_PACK_ROWS);
        assert!(cut.truncated);
        let tabbed = command(
            Extractor::Table {
                columns: vec!["a".into(), "b".into()],
                skip_lines: None,
                separator: Some(Separator::Tab),
            },
            None,
        );
        assert_eq!(
            interpret(&tabbed, true, "one two\tthree\n").rows[0],
            ["one two", "three"]
        );
    }

    #[test]
    fn a_json_path_reads_one_scalar_and_nothing_else() {
        let reader = |path: &str| {
            command(
                Extractor::Json {
                    path: path.to_owned(),
                },
                None,
            )
        };
        let text = r#"{"pool":{"state":"ONLINE","disks":[{"errors":0},{"errors":3}]},"ok":true}"#;
        assert_eq!(
            interpret(&reader("pool.state"), true, text)
                .value
                .as_deref(),
            Some("ONLINE")
        );
        assert_eq!(
            interpret(&reader("pool.disks[1].errors"), true, text)
                .value
                .as_deref(),
            Some("3")
        );
        assert_eq!(
            interpret(&reader("ok"), true, text).value.as_deref(),
            Some("true")
        );
        for path in ["pool", "pool.disks[9].errors", "missing"] {
            assert_eq!(
                interpret(&reader(path), true, text).outcome,
                PackOutcome::Unreadable
            );
        }
        assert_eq!(
            interpret(&reader("pool.state"), true, "not json").outcome,
            PackOutcome::Unreadable
        );
    }
}
