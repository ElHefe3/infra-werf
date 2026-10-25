use crate::*;
use chrono::{DateTime, Utc};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
    params,
};
use serde::Serialize;
use std::{fs::File, io, path::Path, time::Duration};

pub const APPLICATION_ID: i32 = 0x41544c53;
pub const SCHEMA_VERSION: i32 = 2;
const MAX_MIGRATION_BYTES: u64 = 64 * 1024 * 1024;
const SCHEMA: &str = "
CREATE TABLE servers (
 id TEXT PRIMARY KEY, name TEXT NOT NULL, host TEXT NOT NULL UNIQUE COLLATE NOCASE,
 x REAL NOT NULL, y REAL NOT NULL, created_at TEXT NOT NULL, source_key TEXT UNIQUE, report TEXT
);
CREATE TABLE edges (
 id TEXT PRIMARY KEY, source TEXT NOT NULL REFERENCES servers(id), target TEXT NOT NULL REFERENCES servers(id),
 label TEXT NOT NULL, UNIQUE(source,target,label), CHECK(source != target)
);
CREATE TABLE events (id INTEGER PRIMARY KEY, at TEXT NOT NULL, message TEXT NOT NULL, server_id TEXT);
";

fn io_error(_: impl std::fmt::Debug) -> AtlasError {
    AtlasError::new(ErrorCode::IoFailure)
}
fn migration_error(_: impl std::fmt::Debug) -> AtlasError {
    AtlasError::new(ErrorCode::MigrationFailed)
}
pub fn sql_error(error: rusqlite::Error) -> AtlasError {
    let code = match error {
        rusqlite::Error::SqliteFailure(e, _) => match e.code {
            rusqlite::ErrorCode::ConstraintViolation
            | rusqlite::ErrorCode::DatabaseBusy
            | rusqlite::ErrorCode::DatabaseLocked => ErrorCode::Conflict,
            _ => ErrorCode::IoFailure,
        },
        _ => ErrorCode::IoFailure,
    };
    AtlasError::new(code)
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
fn staging(path: &Path) -> Result<tempfile::TempPath, AtlasError> {
    if path.exists() {
        return Err(AtlasError::new(ErrorCode::Conflict));
    }
    if path.file_name().is_none() {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    std::fs::create_dir_all(parent(path)).map_err(io_error)?;
    Ok(tempfile::Builder::new()
        .prefix(".atlas-stage-")
        .tempfile_in(parent(path))
        .map_err(io_error)?
        .into_temp_path())
}
fn publish(path: tempfile::TempPath, target: &Path) -> Result<(), AtlasError> {
    File::options()
        .read(true)
        .write(true)
        .open(&path)
        .and_then(|file| file.sync_all())
        .map_err(io_error)?;
    path.persist_noclobber(target)
        .map_err(|_| AtlasError::new(ErrorCode::Conflict))
}
fn initialise(db: &Connection) -> Result<(), AtlasError> {
    db.execute_batch(SCHEMA).map_err(sql_error)?;
    db.pragma_update(None, "application_id", APPLICATION_ID)
        .map_err(sql_error)?;
    db.pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(sql_error)
}
fn marker(db: &Connection) -> Result<(i32, i32), AtlasError> {
    Ok((
        db.pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(sql_error)?,
        db.pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(sql_error)?,
    ))
}
pub(crate) fn event(db: &Connection, message: &str, sid: Option<&str>) -> Result<(), AtlasError> {
    db.execute(
        "INSERT INTO events(at,message,server_id) VALUES(?,?,?)",
        params![Utc::now().to_rfc3339(), message, sid],
    )
    .map_err(sql_error)?;
    db.execute(
        "DELETE FROM events WHERE id NOT IN (SELECT id FROM events ORDER BY id DESC LIMIT 200)",
        [],
    )
    .map_err(sql_error)?;
    Ok(())
}

pub struct Store {
    pub(crate) db: Connection,
}
impl Store {
    /// Create a separate, marked native file; never initialise an existing DB.
    pub fn create(path: &Path) -> Result<Self, AtlasError> {
        let stage = staging(path)?;
        {
            let db = Connection::open(&stage).map_err(sql_error)?;
            initialise(&db)?;
        }
        publish(stage, path)?;
        Self::open(path)
    }

    /// Retain an SQLite exclusive connection lock so even non-cooperating
    /// SQLite/Python writers cannot write this file while Atlas owns it.
    pub fn open(path: &Path) -> Result<Self, AtlasError> {
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(sql_error)?;
        db.busy_timeout(Duration::from_millis(150))
            .map_err(sql_error)?;
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA locking_mode=EXCLUSIVE; BEGIN EXCLUSIVE;").map_err(sql_error)?;
        if marker(&db)? != (APPLICATION_ID, SCHEMA_VERSION) {
            return Err(AtlasError::new(ErrorCode::IncompatibleVersion));
        }
        db.execute_batch("COMMIT;").map_err(sql_error)?;
        Ok(Self { db })
    }

    pub fn snapshot(&self) -> Result<MapState, AtlasError> {
        self.snapshot_at(Utc::now())
    }
    pub fn snapshot_at(&self, now: DateTime<Utc>) -> Result<MapState, AtlasError> {
        let mut servers = Vec::new();
        for row in raw_servers(&self.db)? {
            let inventory = row
                .report
                .as_ref()
                .map(|s| serde_json::from_str::<BaselineInventory>(s).map_err(io_error))
                .transpose()?;
            let freshness = inventory
                .as_ref()
                .map(|i| Freshness::for_saved(&i.collected_at, now))
                .transpose()?
                .unwrap_or(Freshness::Unknown);
            servers.push(MapServer {
                id: ServerId::parse(&row.id)?,
                name: row.name,
                host: row.host,
                x: row.x,
                y: row.y,
                created_at: row.created_at,
                inventory,
                freshness,
                reachability: HostReachability::default(),
            });
        }
        let edges = read_edges(&self.db)?;
        let events = read_events(&self.db, true)?;
        Ok(MapState {
            contract_version: CONTRACT_VERSION,
            servers,
            edges,
            events,
            served_at: now.to_rfc3339(),
        })
    }

    pub fn add_server(&mut self, input: AddServer) -> Result<ServerId, AtlasError> {
        let name = text(&input.name, 80, false)?;
        let host = hostname(&input.host)?;
        let id = ServerId::generate();
        let tx = self.db.transaction().map_err(sql_error)?;
        let count: i64 = tx
            .query_row("SELECT count(*) FROM servers", [], |r| r.get(0))
            .map_err(sql_error)?;
        tx.execute(
            "INSERT INTO servers VALUES(?,?,?,?,?,?,NULL,NULL)",
            params![
                id.as_str(),
                name,
                host,
                45.0 + (count % 3) as f64 * 300.0,
                65.0 + (count / 3) as f64 * 250.0,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(sql_error)?;
        event(&tx, &format!("Server added: {name}"), Some(id.as_str()))?;
        tx.commit().map_err(sql_error)?;
        Ok(id)
    }

    pub fn update_server(&mut self, id: &ServerId, input: UpdateServer) -> Result<(), AtlasError> {
        input.validate()?;
        let name = input
            .name
            .as_ref()
            .map(|v| text(v, 80, false))
            .transpose()?;
        let host = input.host.as_ref().map(|v| hostname(v)).transpose()?;
        let tx = self.db.transaction().map_err(sql_error)?;
        let changed = tx.execute("UPDATE servers SET name=coalesce(?1,name),host=coalesce(?2,host),x=coalesce(?3,x),y=coalesce(?4,y) WHERE id=?5", params![name, host, input.x, input.y, id.as_str()]).map_err(sql_error)?;
        if changed == 0 {
            return Err(AtlasError::new(ErrorCode::NotFound));
        }
        if name.is_some() || host.is_some() {
            event(&tx, "Server details updated", Some(id.as_str()))?;
        }
        tx.commit().map_err(sql_error)
    }

    pub fn remove_server(&mut self, id: &ServerId) -> Result<(), AtlasError> {
        let tx = self.db.transaction().map_err(sql_error)?;
        tx.execute(
            "DELETE FROM edges WHERE source=?1 OR target=?1",
            [id.as_str()],
        )
        .map_err(sql_error)?;
        if tx
            .execute("DELETE FROM servers WHERE id=?", [id.as_str()])
            .map_err(sql_error)?
            == 0
        {
            return Err(AtlasError::new(ErrorCode::NotFound));
        }
        event(&tx, "Server removed from map", None)?;
        tx.commit().map_err(sql_error)
    }

    pub fn add_relationship(&mut self, input: AddRelationship) -> Result<EdgeId, AtlasError> {
        if input.source == input.target {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        let label = text(&input.label, 80, false)?;
        let id = EdgeId::generate();
        let tx = self.db.transaction().map_err(sql_error)?;
        let count: i64 = tx
            .query_row(
                "SELECT count(*) FROM servers WHERE id IN (?,?)",
                params![input.source.as_str(), input.target.as_str()],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        if count != 2 {
            return Err(AtlasError::new(ErrorCode::NotFound));
        }
        tx.execute(
            "INSERT INTO edges VALUES(?,?,?,?)",
            params![
                id.as_str(),
                input.source.as_str(),
                input.target.as_str(),
                label
            ],
        )
        .map_err(sql_error)?;
        event(
            &tx,
            &format!("Relationship documented: {label}"),
            Some(input.source.as_str()),
        )?;
        tx.commit().map_err(sql_error)?;
        Ok(id)
    }

    pub fn remove_relationship(&mut self, id: &EdgeId) -> Result<(), AtlasError> {
        let tx = self.db.transaction().map_err(sql_error)?;
        if tx
            .execute("DELETE FROM edges WHERE id=?", [id.as_str()])
            .map_err(sql_error)?
            == 0
        {
            return Err(AtlasError::new(ErrorCode::NotFound));
        }
        event(&tx, "Relationship removed from map", None)?;
        tx.commit().map_err(sql_error)
    }
}

pub(crate) struct RawServer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub x: f64,
    pub y: f64,
    pub created_at: String,
    pub source_key: Option<String>,
    pub report: Option<String>,
}
pub(crate) fn raw_servers(db: &Connection) -> Result<Vec<RawServer>, AtlasError> {
    let mut stmt = db.prepare("SELECT id,name,host,x,y,created_at,source_key,report FROM servers ORDER BY created_at,id").map_err(sql_error)?;
    stmt.query_map([], |r| {
        Ok(RawServer {
            id: r.get(0)?,
            name: r.get(1)?,
            host: r.get(2)?,
            x: r.get(3)?,
            y: r.get(4)?,
            created_at: r.get(5)?,
            source_key: r.get(6)?,
            report: r.get(7)?,
        })
    })
    .map_err(sql_error)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(sql_error)
}
fn read_edges(db: &Connection) -> Result<Vec<Relationship>, AtlasError> {
    let mut stmt = db
        .prepare("SELECT id,source,target,label FROM edges ORDER BY rowid")
        .map_err(sql_error)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    rows.into_iter()
        .map(|(id, source, target, label)| {
            Ok(Relationship {
                id: EdgeId::parse(&id)?,
                source: ServerId::parse(&source)?,
                target: ServerId::parse(&target)?,
                label,
            })
        })
        .collect()
}
fn read_events(db: &Connection, limit: bool) -> Result<Vec<Activity>, AtlasError> {
    let sql = if limit {
        "SELECT id,at,message,server_id FROM events ORDER BY id DESC LIMIT 50"
    } else {
        "SELECT id,at,message,server_id FROM events ORDER BY id"
    };
    let mut stmt = db.prepare(sql).map_err(sql_error)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    rows.into_iter()
        .map(|(id, at, message, sid)| {
            Ok(Activity {
                id: id.to_string(),
                at,
                message,
                server_id: sid.as_deref().map(ServerId::parse).transpose()?,
            })
        })
        .collect()
}

fn legacy_snapshot(source: &Path, output: &Path) -> Result<(), AtlasError> {
    legacy_snapshot_bounded(source, output, MAX_MIGRATION_BYTES)
}

fn legacy_snapshot_bounded(source: &Path, output: &Path, limit: u64) -> Result<(), AtlasError> {
    if std::fs::metadata(source).map_err(io_error)?.len() > limit {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    let src =
        Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sql_error)?;
    if marker(&src)? != (0, 1) {
        return Err(AtlasError::new(ErrorCode::IncompatibleVersion));
    }
    let page_size: i64 = src
        .pragma_query_value(None, "page_size", |r| r.get(0))
        .map_err(sql_error)?;
    let page_size =
        u64::try_from(page_size).map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?;
    if page_size == 0 {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    let mut dst = Connection::open(output).map_err(sql_error)?;
    let backup = Backup::new(&src, &mut dst).map_err(sql_error)?;
    let mut completed = false;
    for _ in 0..1000 {
        let step = backup.step(128).map_err(sql_error)?;
        // The main file can be small while committed WAL pages are large.
        // Bound the actual consistent snapshot, not only the source filename.
        if (backup.progress().pagecount as u64).saturating_mul(page_size) > limit {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        match step {
            StepResult::Done => {
                completed = true;
                break;
            }
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => std::thread::sleep(Duration::from_millis(5)),
            _ => return Err(AtlasError::new(ErrorCode::MigrationFailed)),
        }
    }
    drop(backup);
    drop(dst);
    if !completed {
        return Err(AtlasError::new(ErrorCode::Conflict));
    }
    if std::fs::metadata(output).map_err(io_error)?.len() > limit {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    Ok(())
}

/// Restore into a new legacy file only; it never overwrites the original.
pub fn restore_legacy(recovery: &Path, destination: &Path) -> Result<(), AtlasError> {
    let stage = staging(destination)?;
    legacy_snapshot(recovery, &stage)?;
    publish(stage, destination)
}

#[derive(Debug, Serialize)]
pub struct MigrationSummary {
    pub servers: usize,
    pub edges: usize,
    pub events: usize,
}

/// Snapshot read-only, retain full recovery, validate/rebuild in a private
/// staging DB, and publish a separate marked target without clobbering files.
pub fn migrate_legacy(
    source: &Path,
    destination: &Path,
    recovery: &Path,
) -> Result<MigrationSummary, AtlasError> {
    migrate_bounded(source, destination, recovery, MAX_MIGRATION_BYTES)
}

fn migrate_bounded(
    source: &Path,
    destination: &Path,
    recovery: &Path,
    limit: u64,
) -> Result<MigrationSummary, AtlasError> {
    if destination == recovery {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    let stage = staging(destination)?;
    let snapshot = tempfile::Builder::new()
        .prefix(".atlas-legacy-")
        .tempfile_in(parent(destination))
        .map_err(io_error)?
        .into_temp_path();
    legacy_snapshot_bounded(source, &snapshot, limit)?;
    let recovery_stage = staging(recovery)?;
    {
        let mut input = File::open(&snapshot).map_err(io_error)?;
        let mut output = File::options()
            .write(true)
            .open(&recovery_stage)
            .map_err(io_error)?;
        io::copy(&mut input, &mut output).map_err(io_error)?;
        output.sync_all().map_err(io_error)?;
    }
    publish(recovery_stage, recovery)?;
    let old = Connection::open_with_flags(&snapshot, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(migration_error)?;
    let objects: i64 = old.query_row("SELECT count(*) FROM sqlite_schema WHERE type IN ('view','trigger') OR (type='table' AND name NOT GLOB 'sqlite_*' AND name NOT IN ('servers','edges','events'))", [], |r| r.get(0)).map_err(migration_error)?;
    if objects != 0 {
        return Err(AtlasError::new(ErrorCode::MigrationFailed));
    }
    let rows = raw_servers(&old).map_err(migration_error)?;
    let edges = read_edges(&old).map_err(migration_error)?;
    let events = read_events(&old, false).map_err(migration_error)?;
    if rows.len() > 10_000 || edges.len() > 100_000 || events.len() > 10_000 {
        return Err(AtlasError::new(ErrorCode::MigrationFailed));
    }
    let now = Utc::now();
    {
        let mut db = Connection::open(&stage).map_err(migration_error)?;
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")
            .map_err(migration_error)?;
        initialise(&db).map_err(migration_error)?;
        let tx = db.transaction().map_err(migration_error)?;
        for row in &rows {
            ServerId::parse(&row.id).map_err(migration_error)?;
            if text(&row.name, 80, false).map_err(migration_error)? != row.name
                || hostname(&row.host).map_err(migration_error)? != row.host
            {
                return Err(AtlasError::new(ErrorCode::MigrationFailed));
            }
            coordinate(row.x).map_err(migration_error)?;
            coordinate(row.y).map_err(migration_error)?;
            validation::observation_time(&row.created_at, now).map_err(migration_error)?;
            if let Some(key) = &row.source_key {
                text(key, 120, false).map_err(migration_error)?;
            }
            let baseline = row
                .report
                .as_ref()
                .map(|raw| {
                    if raw.len() > inventory::MAX_REPORT_BYTES {
                        return Err(AtlasError::new(ErrorCode::MigrationFailed));
                    }
                    let value = serde_json::from_str(raw).map_err(migration_error)?;
                    serde_json::to_string(
                        &inventory::classify(&value, now).map_err(migration_error)?,
                    )
                    .map_err(migration_error)
                })
                .transpose()?;
            tx.execute(
                "INSERT INTO servers VALUES(?,?,?,?,?,?,?,?)",
                params![
                    row.id,
                    row.name,
                    row.host,
                    row.x,
                    row.y,
                    row.created_at,
                    row.source_key,
                    baseline
                ],
            )
            .map_err(migration_error)?;
        }
        for edge in &edges {
            if text(&edge.label, 80, false).map_err(migration_error)? != edge.label {
                return Err(AtlasError::new(ErrorCode::MigrationFailed));
            }
            tx.execute(
                "INSERT INTO edges VALUES(?,?,?,?)",
                params![
                    edge.id.as_str(),
                    edge.source.as_str(),
                    edge.target.as_str(),
                    edge.label
                ],
            )
            .map_err(migration_error)?;
        }
        for entry in &events {
            text(&entry.message, 4096, false).map_err(migration_error)?;
            validation::observation_time(&entry.at, now).map_err(migration_error)?;
            let id: i64 = entry.id.parse().map_err(migration_error)?;
            tx.execute(
                "INSERT INTO events VALUES(?,?,?,?)",
                params![
                    id,
                    entry.at,
                    entry.message,
                    entry.server_id.as_ref().map(ServerId::as_str)
                ],
            )
            .map_err(migration_error)?;
        }
        tx.commit().map_err(migration_error)?;
        let integrity: String = db
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(migration_error)?;
        if integrity != "ok" {
            return Err(AtlasError::new(ErrorCode::MigrationFailed));
        }
    }
    drop(old);
    publish(stage, destination)?;
    Ok(MigrationSummary {
        servers: rows.len(),
        edges: edges.len(),
        events: events.len(),
    })
}

#[cfg(test)]
mod snapshot_limit_tests {
    use super::*;

    #[test]
    fn wal_snapshot_limit_is_checked_before_recovery_or_native_publication() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("wal.sqlite3");
        let writer = Connection::open(&old).unwrap();
        writer
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
            .unwrap();
        writer.execute_batch(SCHEMA).unwrap();
        writer.pragma_update(None, "user_version", 1).unwrap();
        let report = serde_json::json!({"hostname":"one.home","collected_at":"2026-01-01T00:00:00Z","cpus":4,"memory_total_kib":4000000,"extra":"x".repeat(64*1024)});
        writer.execute("INSERT INTO servers VALUES('one','One','one.home',0,0,'2026-01-01T00:00:00Z',NULL,?)", [report.to_string()]).unwrap();
        let limit = 32 * 1024;
        assert!(std::fs::metadata(&old).unwrap().len() <= limit);
        let before = std::fs::read(&old).unwrap();
        let native = tmp.path().join("native.sqlite3");
        let recovery = tmp.path().join("recovery.sqlite3");
        assert_eq!(
            migrate_bounded(&old, &native, &recovery, limit)
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
        assert!(!native.exists() && !recovery.exists());
        assert_eq!(std::fs::read(&old).unwrap(), before);
        assert_eq!(
            writer
                .query_row("SELECT count(*) FROM servers", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
