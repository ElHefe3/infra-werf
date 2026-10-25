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
use std::{collections::HashSet, fs::File, io::Read, path::Path};

pub const MAX_REPORT_BYTES: usize = 1024 * 1024;

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
        cpus: count("cpus")?,
        memory_total_kib: count("memory_total_kib")?,
        services,
        containers,
    })
}
