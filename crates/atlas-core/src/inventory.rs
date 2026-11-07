use crate::ssh::SshProfile;
use crate::{
    AtlasError, BaselineInventory, ContainerSummary, ErrorCode, ServiceSummary, validation,
};
use crate::{
    ServerId, Store,
    store::{event, sql_error},
};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs::File, io::Read, path::Path};

pub const MAX_REPORT_BYTES: usize = 1024 * 1024;
const MAX_REFRESH_OUTPUT_BYTES: usize = 64 * 1024;

/// Wait out only a small remote clock lead, rather than accepting future data.
/// The caller keeps cancellation/session checks active during this bounded wait.
pub(crate) fn remote_clock_delay(output: &str, now: DateTime<Utc>) -> Option<std::time::Duration> {
    if output.len() > MAX_REFRESH_OUTPUT_BYTES || !output.starts_with("ATLAS_BASELINE_V1\nTIME\t") {
        return None;
    }
    if output
        .lines()
        .filter(|line| line.starts_with("TIME\t"))
        .count()
        != 1
    {
        return None;
    }
    let value = output.lines().nth(1)?.strip_prefix("TIME\t")?;
    if value.len() > 64 {
        return None;
    }
    let source = DateTime::parse_from_rfc3339(value)
        .ok()?
        .with_timezone(&Utc);
    let ahead = source.signed_duration_since(now);
    if ahead > chrono::Duration::zero() && ahead <= chrono::Duration::seconds(5) {
        ahead.to_std().ok()
    } else {
        None
    }
}

/// Fixed read-only Linux inventory allowlist. Only selected fields are emitted;
/// the remote command's descriptions, logs, process list and configuration are
/// never sent back to Atlas.
pub(crate) const REMOTE_BASELINE_COMMAND: &str = r##"
set -o pipefail
LC_ALL=C
LANG=C
export LC_ALL LANG
system_docker() {
  unset DOCKER_HOST DOCKER_CONTEXT DOCKER_TLS_VERIFY DOCKER_CERT_PATH
  command docker --host unix:///var/run/docker.sock "$@"
}
printf 'ATLAS_BASELINE_V1\n'
printf 'TIME\t'; date -u '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || true
if ! inventory_host=$(hostname 2>/dev/null); then
  IFS= read -r inventory_host < /proc/sys/kernel/hostname || inventory_host=''
fi
inventory_cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null) || inventory_cpus=$(awk '/^processor[[:space:]]*:/ {count++} END {if (count) print count; else exit 1}' /proc/cpuinfo 2>/dev/null) || inventory_cpus=''
printf 'HOST\t%s\n' "$inventory_host"
printf 'CPUS\t%s\n' "$inventory_cpus"
printf 'MEMORY_KIB\t'; awk '/^MemTotal:/ {print $2; found=1; exit} END {if (!found) exit 1}' /proc/meminfo 2>/dev/null || true
if command -v systemctl >/dev/null 2>&1; then
  if systemctl --system list-units --all --type=service --no-legend --no-pager --plain --full 2>/dev/null | awk -v scope=system 'NF >= 4 {printf "SERVICE\t%s\t%s\t%s\t%s\n", scope, $1, $3, $4; next} {bad=1} END {if (bad) exit 1}'; then
    printf 'SYSTEM_SERVICES_OK\n'
  else
    printf 'SYSTEM_SERVICES_PARTIAL\n'
  fi
  if systemctl --user list-units --all --type=service --no-legend --no-pager --plain --full 2>/dev/null | awk -v scope=user 'NF >= 4 {printf "SERVICE\t%s\t%s\t%s\t%s\n", scope, $1, $3, $4; next} {bad=1} END {if (bad) exit 1}'; then
    printf 'USER_SERVICES_OK\n'
  else
    printf 'USER_SERVICES_PARTIAL\n'
  fi
else
  printf 'SYSTEM_SERVICES_PARTIAL\nUSER_SERVICES_PARTIAL\n'
fi
engines=0
containers_ok=1
for engine in podman docker; do
  if command -v "$engine" >/dev/null 2>&1; then
    if [ "$engine" = docker ] && command -v podman >/dev/null 2>&1; then
      docker_version=$(docker --version 2>/dev/null) || docker_version=''
      case "$docker_version" in
        'podman version '*) continue ;;
      esac
    fi
    engines=$((engines + 1))
    scope=user
    [ "$engine" = docker ] && scope=system
    if [ "$engine" = docker ]; then
      if system_docker ps -a --no-trunc --format '{{.Names}}|{{.Image}}|{{.State}}' 2>/dev/null | awk -F '|' -v scope="$scope" -v engine="$engine" 'NF == 3 && $1 != "" && $3 != "" {printf "CONTAINER\t%s\t%s\t%s\t%s\t%s\n", scope, engine, $1, $2, $3; next} {bad=1} END {if (bad) exit 1}'; then
        :
      else
        containers_ok=0
      fi
    else
      if "$engine" ps -a --no-trunc --format '{{.Names}}|{{.Image}}|{{.State}}' 2>/dev/null | awk -F '|' -v scope="$scope" -v engine="$engine" 'NF == 3 && $1 != "" && $3 != "" {printf "CONTAINER\t%s\t%s\t%s\t%s\t%s\n", scope, engine, $1, $2, $3; next} {bad=1} END {if (bad) exit 1}'; then
        :
      else
        containers_ok=0
      fi
    fi
  fi
done
if [ "$engines" -eq 0 ]; then
  printf 'CONTAINERS_NONE\n'
elif [ "$containers_ok" -eq 1 ]; then
  printf 'CONTAINERS_OK\n'
else
  printf 'CONTAINERS_PARTIAL\n'
fi
printf 'END\n'
"##;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InventoryProbe {
    pub inventory: Option<BaselineInventory>,
    pub source_collected_at: Option<String>,
    pub source_binding: Option<String>,
}

fn partial_probe(source_collected_at: Option<String>) -> InventoryProbe {
    InventoryProbe {
        inventory: None,
        source_collected_at,
        source_binding: None,
    }
}

/// Opaque binding to the resolved SSH alias/endpoint/account and authenticated
/// server key. It is evidence for freshness comparison, not an auth token.
pub fn authenticated_source_binding(profile: &SshProfile, fingerprint: &str) -> String {
    let port = profile.port.to_string();
    let identity_files = profile.identity_files.to_string();
    let parts = [
        profile.alias.as_str(),
        profile.hostname.as_str(),
        profile.user.as_str(),
        port.as_str(),
        identity_files.as_str(),
        fingerprint,
    ];
    let mut digest = Sha256::new();
    digest.update(b"ATLAS_INVENTORY_SOURCE_V1\0");
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    let mut binding = String::from("src1_");
    for byte in digest.finalize() {
        use std::fmt::Write as _;
        let _ = write!(binding, "{byte:02x}");
    }
    binding
}

/// Parse the narrow line protocol. Unknown, malformed, duplicate or incomplete
/// output is reported as partial and cannot become a persisted baseline.
pub(crate) fn parse_remote_baseline(output: &str, now: DateTime<Utc>) -> InventoryProbe {
    if output.len() > MAX_REFRESH_OUTPUT_BYTES || !output.ends_with('\n') {
        return partial_probe(None);
    }
    let mut source_collected_at: Option<String> = None;
    let mut hostname: Option<String> = None;
    let mut cpus: Option<u64> = None;
    let mut memory_total_kib: Option<u64> = None;
    let mut services = Vec::new();
    let mut containers = Vec::new();
    let mut service_keys = HashSet::new();
    let mut container_keys = HashSet::new();
    let (mut system_services, mut user_services, mut containers_complete) = (false, false, false);
    let (mut containers_empty, mut ended, mut valid) = (false, false, true);
    let mut lines = output.split_terminator('\n');
    if lines.next() != Some("ATLAS_BASELINE_V1") {
        return partial_probe(None);
    }
    for line in lines {
        if ended || line.contains('\r') {
            valid = false;
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        match fields.as_slice() {
            ["TIME", value] if source_collected_at.is_none() => {
                let checked = validation::observation_time(value, now);
                if checked.is_ok() {
                    source_collected_at = Some((*value).to_owned());
                } else {
                    valid = false;
                }
            }
            ["HOST", value] if hostname.is_none() => hostname = Some((*value).to_owned()),
            ["CPUS", value] if cpus.is_none() => match value.parse() {
                Ok(value) => cpus = Some(value),
                Err(_) => valid = false,
            },
            ["MEMORY_KIB", value] if memory_total_kib.is_none() => match value.parse() {
                Ok(value) => memory_total_kib = Some(value),
                Err(_) => valid = false,
            },
            ["SERVICE", scope, name, state, substate] => {
                if services.len() >= 1024
                    || !matches!(*scope, "system" | "user")
                    || !service_keys.insert(((*scope).to_owned(), (*name).to_owned()))
                {
                    valid = false;
                } else {
                    services.push(serde_json::json!({
                        "scope": scope,
                        "name": name,
                        "state": state,
                        "substate": substate
                    }));
                }
            }
            ["SYSTEM_SERVICES_OK"] if !system_services => system_services = true,
            ["USER_SERVICES_OK"] if !user_services => user_services = true,
            ["SYSTEM_SERVICES_PARTIAL" | "USER_SERVICES_PARTIAL"] => valid = false,
            ["CONTAINER", scope, engine, name, image, state] => {
                if containers.len() >= 1024
                    || containers_empty
                    || !matches!(*scope, "system" | "user")
                    || !matches!(*engine, "podman" | "docker")
                    || !container_keys.insert((
                        (*scope).to_owned(),
                        (*engine).to_owned(),
                        (*name).to_owned(),
                    ))
                {
                    valid = false;
                } else {
                    containers.push(serde_json::json!({
                        "scope": scope,
                        "engine": engine,
                        "name": name,
                        "image": image,
                        "state": state
                    }));
                }
            }
            ["CONTAINERS_OK"] if !containers_complete && !containers_empty => {
                containers_complete = true
            }
            ["CONTAINERS_NONE"]
                if !containers_complete && !containers_empty && containers.is_empty() =>
            {
                containers_empty = true
            }
            ["CONTAINERS_PARTIAL"] => valid = false,
            ["END"] => ended = true,
            _ => valid = false,
        }
    }
    let collected_at = source_collected_at.clone();
    if !valid
        || !ended
        || !system_services
        || !user_services
        || !(containers_complete || containers_empty)
    {
        return partial_probe(collected_at);
    }
    let Some(hostname) = hostname else {
        return partial_probe(collected_at);
    };
    let Some(cpus) = cpus else {
        return partial_probe(collected_at);
    };
    let Some(memory_total_kib) = memory_total_kib else {
        return partial_probe(collected_at);
    };
    let Some(collected_at) = source_collected_at else {
        return partial_probe(None);
    };
    let raw = serde_json::json!({
        "hostname": hostname,
        "collected_at": collected_at,
        "cpus": cpus,
        "memory_total_kib": memory_total_kib,
        "services": services,
        "containers": containers,
    });
    match classify(&raw, now) {
        Ok(inventory) => InventoryProbe {
            source_collected_at: Some(inventory.collected_at.clone()),
            inventory: Some(inventory),
            source_binding: None,
        },
        Err(_) => partial_probe(Some(collected_at)),
    }
}

/// Raw input exists only until validation/classification, never as a wire DTO.
pub struct ImportRecord {
    pub source_key: String,
    pub default_name: String,
    pub report: Value,
}

pub fn load_reports(directory: &Path) -> Result<Vec<ImportRecord>, AtlasError> {
    if !directory.is_dir() {
        return Err(invalid());
    }
    let root = directory
        .canonicalize()
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
    let mut reports = Vec::new();
    for (key, name) in [
        ("git", "Core & data"),
        ("runner", "Infrastructure"),
        ("game", "Compute & apps"),
        ("archpc", "Communications"),
    ] {
        let path = root.join(format!("{key}.json"));
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(AtlasError::new(ErrorCode::IoFailure)),
        };
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAX_REPORT_BYTES as u64
        {
            return Err(invalid());
        }
        if !path
            .canonicalize()
            .map_err(|_| invalid())?
            .starts_with(&root)
        {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
            .take(MAX_REPORT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
        if bytes.len() > MAX_REPORT_BYTES {
            return Err(invalid());
        }
        let record: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        reports.push(ImportRecord {
            source_key: key.into(),
            default_name: name.into(),
            report: record.get("report").ok_or_else(invalid)?.clone(),
        });
    }
    Ok(reports)
}

impl Store {
    /// Replace only the approved inventory column for an existing server ID.
    /// Owner-authored connection fields, layout and relationships are untouched.
    pub fn refresh_inventory(
        &mut self,
        id: &ServerId,
        inventory: &BaselineInventory,
        source_binding: &str,
    ) -> Result<bool, AtlasError> {
        let raw = serde_json::to_value(inventory)
            .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?;
        let mut inventory = classify(&raw, Utc::now())?;
        let tx = self.db.transaction().map_err(sql_error)?;
        let previous: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT host,report FROM servers WHERE id=?",
                [id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sql_error)?;
        let Some((source_host, previous_report)) = previous else {
            return Err(AtlasError::new(ErrorCode::NotFound));
        };
        if source_binding.len() != 69
            || !source_binding.starts_with("src1_")
            || !source_binding[5..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        inventory.source_host = Some(source_host.clone());
        inventory.source_binding = Some(source_binding.to_owned());
        let change_message = inventory_changes(previous_report.as_deref(), &inventory);
        if let Some(previous_report) = previous_report {
            let mut previous: BaselineInventory = serde_json::from_str(&previous_report)
                .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
            let previous_time = validation::observation_time(&previous.collected_at, Utc::now())?;
            let incoming_time = validation::observation_time(&inventory.collected_at, Utc::now())?;
            if previous_time > incoming_time {
                return Ok(false);
            }
            if previous_time == incoming_time {
                // Rebind an equal, otherwise identical observation only after
                // a complete refresh through this exact authenticated SSH profile.
                // Older/imported records are never implicitly trusted.
                let mut previous_without_provenance = previous.clone();
                previous_without_provenance.source_host = None;
                previous_without_provenance.source_binding = None;
                let mut incoming_without_provenance = inventory.clone();
                incoming_without_provenance.source_host = None;
                incoming_without_provenance.source_binding = None;
                if previous_without_provenance == incoming_without_provenance
                    && (previous.source_host.as_deref() != Some(source_host.as_str())
                        || previous.source_binding.as_deref() != Some(source_binding))
                {
                    previous.source_host = Some(source_host);
                    previous.source_binding = Some(source_binding.to_owned());
                    let encoded = serde_json::to_string(&previous)
                        .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?;
                    tx.execute(
                        "UPDATE servers SET report=? WHERE id=?",
                        rusqlite::params![encoded, id.as_str()],
                    )
                    .map_err(sql_error)?;
                    tx.commit().map_err(sql_error)?;
                }
                return Ok(false);
            }
        }
        let encoded = serde_json::to_string(&inventory)
            .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?;
        if tx
            .execute(
                "UPDATE servers SET report=? WHERE id=?",
                rusqlite::params![encoded, id.as_str()],
            )
            .map_err(sql_error)?
            != 1
        {
            return Err(AtlasError::new(ErrorCode::NotFound));
        }
        if let Some(message) = change_message {
            event(&tx, &message, Some(id.as_str()))?;
        }
        tx.commit().map_err(sql_error)?;
        Ok(true)
    }

    pub fn import_reports(&mut self, directory: &Path) -> Result<usize, AtlasError> {
        self.import_batch(&load_reports(directory)?)
    }

    /// Validate the entire bounded batch before writing. Stable source identity
    /// wins over editable connection hostnames; ambiguous adoption is rejected.
    pub fn import_batch(&mut self, reports: &[ImportRecord]) -> Result<usize, AtlasError> {
        if reports.len() > 64 {
            return Err(invalid());
        }
        let now = Utc::now();
        let mut keys = HashSet::new();
        let mut hosts = HashSet::new();
        let mut approved = Vec::new();
        for record in reports {
            let key = validation::text(&record.source_key, 120, false)?;
            let name = validation::text(&record.default_name, 80, false)?;
            let baseline = classify(&record.report, now)?;
            if !keys.insert(key.clone()) || !hosts.insert(baseline.hostname.to_ascii_lowercase()) {
                return Err(invalid());
            }
            approved.push((key, name, baseline));
        }
        let tx = self.db.transaction().map_err(sql_error)?;
        let mut count = 0;
        for (index, (key, name, report)) in approved.into_iter().enumerate() {
            type Existing = (String, Option<String>, Option<String>);
            let by_key: Option<Existing> = tx
                .query_row(
                    "SELECT id,source_key,report FROM servers WHERE source_key=?",
                    [&key],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()
                .map_err(sql_error)?;
            let by_host: Option<Existing> = tx
                .query_row(
                    "SELECT id,source_key,report FROM servers WHERE host=? COLLATE NOCASE",
                    [&report.hostname],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()
                .map_err(sql_error)?;
            if let (Some(a), Some(b)) = (&by_key, &by_host) {
                if a.0 != b.0 {
                    return Err(AtlasError::new(ErrorCode::Conflict));
                }
            }
            if by_key.is_none()
                && by_host
                    .as_ref()
                    .and_then(|r| r.1.as_ref())
                    .is_some_and(|existing| existing != &key)
            {
                return Err(AtlasError::new(ErrorCode::Conflict));
            }
            let row = by_key.or(by_host);
            if let Some((_, _, Some(previous))) = &row {
                let previous: BaselineInventory = serde_json::from_str(previous)
                    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
                if validation::observation_time(&previous.collected_at, now)?
                    >= validation::observation_time(&report.collected_at, now)?
                {
                    continue;
                }
            }
            let encoded = serde_json::to_string(&report).map_err(|_| invalid())?;
            let id = if let Some((id, _, _)) = row {
                tx.execute(
                    "UPDATE servers SET source_key=?,report=? WHERE id=?",
                    params![key, encoded, id],
                )
                .map_err(sql_error)?;
                id
            } else {
                let id = ServerId::generate();
                tx.execute(
                    "INSERT INTO servers VALUES(?,?,?,?,?,?,?,?)",
                    params![
                        id.as_str(),
                        name,
                        report.hostname,
                        45.0 + (index % 2) as f64 * 320.0,
                        65.0 + (index / 2) as f64 * 260.0,
                        now.to_rfc3339(),
                        key,
                        encoded
                    ],
                )
                .map_err(sql_error)?;
                id.as_str().to_owned()
            };
            event(
                &tx,
                &format!("Saved inventory imported ({})", report.collected_at),
                Some(&id),
            )?;
            count += 1;
        }
        tx.commit().map_err(sql_error)?;
        Ok(count)
    }
}

fn inventory_changes(previous: Option<&str>, current: &BaselineInventory) -> Option<String> {
    let Some(previous) = previous.and_then(|p| serde_json::from_str::<BaselineInventory>(p).ok())
    else {
        return Some("Initial approved inventory collected".into());
    };
    fn items(inventory: &BaselineInventory) -> std::collections::BTreeMap<String, String> {
        inventory
            .services
            .iter()
            .map(|s| {
                (
                    format!("service {}/{}", s.scope, s.name),
                    format!("{}/{}", s.state, s.substate),
                )
            })
            .chain(inventory.containers.iter().map(|c| {
                (
                    format!("container {}/{}/{}", c.scope, c.engine, c.name),
                    format!("{} {}", c.state, c.image),
                )
            }))
            .collect()
    }
    let old = items(&previous);
    let new = items(current);
    let mut changes = Vec::new();
    for (key, value) in &new {
        match old.get(key) {
            None => changes.push(format!("added {key}")),
            Some(old) if old != value => changes.push(format!("{key}: {old} → {value}")),
            _ => {}
        }
    }
    for key in old.keys() {
        if !new.contains_key(key) {
            changes.push(format!("removed {key}"));
        }
    }
    if previous.cpus != current.cpus
        || previous.memory_total_kib != current.memory_total_kib
        || previous.hostname != current.hostname
    {
        changes.push("host details changed".into());
    }
    if changes.is_empty() {
        None
    } else {
        Some(format!(
            "Inventory: {} change(s) · {}{}",
            changes.len(),
            changes
                .iter()
                .take(5)
                .map(|s| s.chars().take(160).collect::<String>())
                .collect::<Vec<_>>()
                .join("; "),
            if changes.len() > 5 { "; …" } else { "" }
        ))
    }
}

fn invalid() -> AtlasError {
    AtlasError::new(ErrorCode::InvalidInput)
}

fn field(row: &Value, name: &str, required: bool) -> Result<String, AtlasError> {
    match row.get(name) {
        Some(Value::String(value)) => validation::text(value, 512, !required),
        None if !required => Ok(String::new()),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;
    #[test]
    fn inventory_history_ignores_clock_only_refreshes_and_describes_changes() {
        let now = Utc::now();
        let probe = parse_remote_baseline(
            &complete_output(&timestamp(now - chrono::Duration::seconds(5))),
            now,
        );
        let old = probe.inventory.unwrap();
        let mut next = old.clone();
        next.collected_at = timestamp(now - chrono::Duration::seconds(1));
        let encoded = serde_json::to_string(&old).unwrap();
        assert!(inventory_changes(Some(&encoded), &next).is_none());
        next.services[0].state = "failed".into();
        let message = inventory_changes(Some(&encoded), &next).unwrap();
        assert!(message.contains("ssh.service"));
        assert!(message.contains("failed"));
        next.services.remove(0);
        assert!(
            inventory_changes(Some(&encoded), &next)
                .unwrap()
                .contains("removed")
        );
    }
    use chrono::SecondsFormat;

    fn timestamp(at: DateTime<Utc>) -> String {
        at.to_rfc3339_opts(SecondsFormat::Secs, true)
    }

    fn complete_output(at: &str) -> String {
        format!(
            "ATLAS_BASELINE_V1\nTIME\t{at}\nHOST\tfixture-node.home\nCPUS\t4\nMEMORY_KIB\t4000000\nSERVICE\tsystem\tssh.service\tactive\trunning\nSERVICE\tuser\tworker.service\tinactive\tdead\nSYSTEM_SERVICES_OK\nUSER_SERVICES_OK\nCONTAINER\tuser\tpodman\tapi\tlocalhost/api:v1\trunning\nCONTAINERS_OK\nEND\n"
        )
    }

    #[test]
    fn a_small_source_clock_lead_is_waited_out_without_accepting_future_data() {
        let now = Utc::now();
        let future = complete_output(&timestamp(now + chrono::Duration::seconds(2)));
        let delay = remote_clock_delay(&future, now).unwrap();
        assert!(delay <= std::time::Duration::from_secs(2));
        assert!(parse_remote_baseline(&future, now).inventory.is_none());
        assert!(
            parse_remote_baseline(&future, now + chrono::Duration::seconds(3))
                .inventory
                .is_some()
        );
        assert!(
            remote_clock_delay(
                &complete_output(&timestamp(now - chrono::Duration::seconds(2))),
                now
            )
            .is_none()
        );
        assert!(
            remote_clock_delay(
                &complete_output(&timestamp(now + chrono::Duration::hours(1))),
                now
            )
            .is_none()
        );
        assert!(
            remote_clock_delay("untrusted preamble\nTIME\t2026-01-01T00:00:00Z", now).is_none()
        );
        assert!(
            remote_clock_delay(
                &future.replace("HOST\t", &format!("TIME\t{}\nHOST\t", timestamp(now))),
                now
            )
            .is_none()
        );
    }

    #[test]
    fn complete_remote_report_is_reconstructed_from_only_approved_fields() {
        let now = Utc::now();
        let collected_at = timestamp(now - chrono::Duration::seconds(2));
        let probe = parse_remote_baseline(&complete_output(&collected_at), now);
        let inventory = probe.inventory.expect("complete approved report");
        assert_eq!(inventory.hostname, "fixture-node.home");
        assert_eq!(inventory.collected_at, collected_at);
        assert_eq!(inventory.cpus, 4);
        assert_eq!(inventory.memory_total_kib, 4_000_000);
        assert_eq!(inventory.services.len(), 2);
        assert_eq!(inventory.containers.len(), 1);
        let encoded = serde_json::to_string(&inventory).unwrap();
        assert!(!encoded.contains("description"));
        assert!(!encoded.contains("raw"));
    }

    #[test]
    fn malformed_partial_future_and_unapproved_remote_rows_never_become_inventory() {
        let now = Utc::now();
        let collected_at = timestamp(now - chrono::Duration::seconds(2));
        for output in [
            complete_output(&collected_at).replace("USER_SERVICES_OK\n", ""),
            complete_output(&collected_at).replace("END\n", "RAW\tprivate-marker\nEND\n"),
            complete_output(&timestamp(now + chrono::Duration::hours(1))),
            complete_output(&collected_at).replace("CPUS\t4\n", "CPUS\tinvalid\nCPUS\t4\n"),
            complete_output(&collected_at).replace(
                "MEMORY_KIB\t4000000\n",
                "MEMORY_KIB\tinvalid\nMEMORY_KIB\t4000000\n",
            ),
            complete_output(&collected_at).replace(
                "CONTAINER\tuser\tpodman\tapi\tlocalhost/api:v1\trunning\n",
                "CONTAINER\tuser\tpodman\tapi\tlocalhost/api:v1\trunning\nCONTAINER\tuser\tpodman\tapi\tlocalhost/api:v1\trunning\n",
            ),
            complete_output(&collected_at).replace(
                "CONTAINER\tuser\tpodman\tapi\tlocalhost/api:v1\trunning\n",
                "CONTAINERS_NONE\nCONTAINER\tuser\tpodman\tapi\tlocalhost/api:v1\trunning\n",
            ),
        ] {
            let probe = parse_remote_baseline(&output, now);
            assert!(probe.inventory.is_none());
            let encoded = serde_json::to_string(&probe).unwrap();
            assert!(!encoded.contains("private-marker"));
        }
    }
}

/// Reconstruct the approved baseline; excluded/raw fields are never returned.
pub fn classify(raw: &Value, now: DateTime<Utc>) -> Result<BaselineInventory, AtlasError> {
    if serde_json::to_vec(raw).map_err(|_| invalid())?.len() > MAX_REPORT_BYTES {
        return Err(invalid());
    }
    if !raw.is_object() {
        return Err(invalid());
    }
    let hostname = validation::hostname(
        raw.get("hostname")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
    )?;
    let collected_at = validation::text(
        raw.get("collected_at")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
        64,
        false,
    )?;
    validation::observation_time(&collected_at, now)?;
    let count = |key: &str| -> Result<u64, AtlasError> {
        raw.get(key)
            .and_then(Value::as_u64)
            .filter(|n| *n > 0 && *n < 1_000_000_000_000)
            .ok_or_else(invalid)
    };
    let rows = |key: &str| -> Result<&[Value], AtlasError> {
        match raw.get(key) {
            None => Ok(&[]),
            Some(Value::Array(rows)) if rows.len() <= 1024 && rows.iter().all(Value::is_object) => {
                Ok(rows)
            }
            _ => Err(invalid()),
        }
    };
    let services = rows("services")?
        .iter()
        .map(|row| {
            Ok(ServiceSummary {
                scope: field(row, "scope", false)?,
                name: field(row, "name", true)?,
                state: field(row, "state", false)?,
                substate: field(row, "substate", false)?,
            })
        })
        .collect::<Result<Vec<_>, AtlasError>>()?;
    let containers = rows("containers")?
        .iter()
        .map(|row| {
            Ok(ContainerSummary {
                scope: field(row, "scope", false)?,
                engine: field(row, "engine", false)?,
                name: field(row, "name", true)?,
                image: field(row, "image", false)?,
                state: field(row, "state", false)?,
            })
        })
        .collect::<Result<Vec<_>, AtlasError>>()?;
    Ok(BaselineInventory {
        hostname,
        collected_at,
        source_host: None,
        source_binding: None,
        cpus: count("cpus")?,
        memory_total_kib: count("memory_total_kib")?,
        services,
        containers,
    })
}
