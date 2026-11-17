use atlas_core::*;
use rusqlite::{Connection, params};
use serde_json::json;
use std::path::Path;

fn add(store: &mut Store, name: &str, host: &str) -> ServerId {
    store
        .add_server(AddServer {
            name: name.into(),
            host: host.into(),
        })
        .unwrap()
}
fn relation(a: &ServerId, b: &ServerId) -> AddRelationship {
    AddRelationship {
        kind: None,
        source: a.clone(),
        target: b.clone(),
        label: "Backs up to".into(),
    }
}
fn legacy(path: &Path) {
    let db = Connection::open(path).unwrap();
    db.execute_batch("CREATE TABLE servers(id TEXT PRIMARY KEY,name TEXT NOT NULL,host TEXT NOT NULL UNIQUE COLLATE NOCASE,x REAL NOT NULL,y REAL NOT NULL,created_at TEXT NOT NULL,source_key TEXT UNIQUE,report TEXT);
    CREATE TABLE edges(id TEXT PRIMARY KEY,source TEXT NOT NULL REFERENCES servers(id),target TEXT NOT NULL REFERENCES servers(id),label TEXT NOT NULL,UNIQUE(source,target,label),CHECK(source!=target));
    CREATE TABLE events(id INTEGER PRIMARY KEY,at TEXT NOT NULL,message TEXT NOT NULL,server_id TEXT); PRAGMA user_version=1;").unwrap();
    let report = json!({"hostname":"observed.home","collected_at":"2026-01-01T00:00:00+00:00","cpus":4,"memory_total_kib":4000000,
        "services":[{"name":"ssh.service","scope":"system","state":"active","substate":"running","boot":"enabled","restart":"always","raw":"never-persist-raw"}],
        "containers":[],"warnings":["never-persist-raw"],"private_key":"never-persist-raw"});
    db.execute("INSERT INTO servers VALUES('stable-a','Owner name','edited.home',123,-45,'2026-01-01T00:00:00+00:00','git',?)", [report.to_string()]).unwrap();
    db.execute("INSERT INTO servers VALUES('stable-b','Second','second.home',456,789,'2026-01-01T00:00:00+00:00',NULL,NULL)", []).unwrap();
    db.execute(
        "INSERT INTO edges VALUES('stable-edge','stable-a','stable-b','Backs up to')",
        [],
    )
    .unwrap();
    db.execute("INSERT INTO events VALUES(42,'2026-01-01T00:00:00+00:00','Server details updated','stable-a')", []).unwrap();
}

#[test]
fn crud_directional_edges_validation_and_restart_share_one_store() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("native.sqlite3");
    let mut store = Store::create(&path).unwrap();
    let a = add(&mut store, "A", "alpha.home");
    let b = add(&mut store, "B", "192.0.2.5");
    store
        .update_server(
            &a,
            UpdateServer {
                name: Some("Renamed".into()),
                host: Some("edited.home".into()),
                x: Some(500.0),
                y: Some(-10.0),
            },
        )
        .unwrap();
    let edge = store.add_relationship(relation(&a, &b)).unwrap();
    let reverse = store.add_relationship(relation(&b, &a)).unwrap();
    assert_eq!(
        store.add_relationship(relation(&a, &b)).unwrap_err().code,
        ErrorCode::Conflict
    );
    assert_eq!(
        store.add_relationship(relation(&a, &a)).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        store
            .add_relationship(relation(&a, &ServerId::parse("missing").unwrap()))
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        store
            .add_server(AddServer {
                name: "Duplicate".into(),
                host: "EDITED.home".into()
            })
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert!(
        store
            .update_server(
                &a,
                UpdateServer {
                    x: Some(f64::NAN),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(
        store
            .update_server(
                &a,
                UpdateServer {
                    x: Some(10001.0),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(
        store
            .update_server(
                &a,
                UpdateServer {
                    host: Some("ssh://bad".into()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    store.remove_relationship(&edge).unwrap();
    drop(store);
    let mut reopened = Store::open(&path).unwrap();
    let state = reopened.snapshot().unwrap();
    let server = state.servers.iter().find(|s| s.id == a).unwrap();
    assert_eq!(
        (&server.name, &server.host, server.x, server.y),
        (&"Renamed".into(), &"edited.home".into(), 500.0, -10.0)
    );
    assert_eq!(state.edges.len(), 1);
    assert_eq!(state.edges[0].id, reverse);
    assert_eq!(server.freshness, Freshness::Unknown);
    assert_eq!(server.reachability.state, Reachability::Unknown);
    reopened.remove_server(&a).unwrap();
    assert!(reopened.snapshot().unwrap().edges.is_empty());
    assert_eq!(reopened.snapshot().unwrap().servers.len(), 1);
}

#[test]
fn native_owner_rejects_legacy_and_blocks_other_sqlite_writers_until_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old.sqlite3");
    legacy(&old);
    let bytes = std::fs::read(&old).unwrap();
    assert_eq!(
        Store::open(&old).err().unwrap().code,
        ErrorCode::IncompatibleVersion
    );
    assert_eq!(std::fs::read(&old).unwrap(), bytes);
    let native = tmp.path().join("native.sqlite3");
    let mut owner = Store::create(&native).unwrap();
    add(&mut owner, "A", "a.home");
    let other = Connection::open(&native).and_then(|c| {
        c.busy_timeout(std::time::Duration::from_millis(1))?;
        c.execute("UPDATE servers SET name='Unwanted writer'", [])
            .map(|_| ())
    });
    assert!(other.is_err());
    assert_eq!(
        Store::open(&native).err().unwrap().code,
        ErrorCode::Conflict
    );
    drop(owner);
    assert_eq!(
        Store::open(&native).unwrap().snapshot().unwrap().servers[0].name,
        "A"
    );
}

#[test]
fn migration_preserves_owner_state_and_full_recovery_but_exposes_only_baseline() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old.sqlite3");
    legacy(&old);
    let before = std::fs::read(&old).unwrap();
    let native = tmp.path().join("native.sqlite3");
    let recovery = tmp.path().join("recovery.sqlite3");
    let result = migrate_legacy(&old, &native, &recovery).unwrap();
    assert_eq!((result.servers, result.edges, result.events), (2, 1, 1));
    assert_eq!(std::fs::read(&old).unwrap(), before);
    let store = Store::open(&native).unwrap();
    let state = store.snapshot().unwrap();
    let a = state
        .servers
        .iter()
        .find(|s| s.id.as_str() == "stable-a")
        .unwrap();
    assert_eq!(
        (&a.name, &a.host, a.x, a.y),
        (&"Owner name".into(), &"edited.home".into(), 123.0, -45.0)
    );
    assert_eq!(a.inventory.as_ref().unwrap().hostname, "observed.home");
    assert_eq!(
        a.inventory.as_ref().unwrap().collected_at,
        "2026-01-01T00:00:00+00:00"
    );
    assert_eq!(a.freshness, Freshness::Stale);
    assert_eq!(a.reachability.state, Reachability::Unknown);
    assert_eq!(state.edges[0].id.as_str(), "stable-edge");
    assert_eq!(state.events[0].id, "42");
    let overview = serde_json::to_string(&state).unwrap();
    for excluded in [
        "never-persist-raw",
        "restart",
        "boot",
        "warnings",
        "private_key",
    ] {
        assert!(!overview.contains(excluded));
    }
    drop(store);
    let original_report: String = Connection::open(&recovery)
        .unwrap()
        .query_row("SELECT report FROM servers WHERE id='stable-a'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(original_report.contains("never-persist-raw"));
    let restored = tmp.path().join("restored.sqlite3");
    restore_legacy(&recovery, &restored).unwrap();
    let db = Connection::open(&restored).unwrap();
    assert_eq!(
        db.pragma_query_value::<i32, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        1
    );
    let report: String = db
        .query_row("SELECT report FROM servers WHERE id='stable-a'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(report, original_report);
    assert_eq!(
        db.query_row("SELECT count(*) FROM edges", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn partial_migration_failure_keeps_source_recovery_and_existing_destinations() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old.sqlite3");
    legacy(&old);
    {
        let db = Connection::open(&old).unwrap();
        db.execute("UPDATE servers SET x=10001 WHERE id='stable-b'", [])
            .unwrap();
    }
    let before = std::fs::read(&old).unwrap();
    let native = tmp.path().join("native.sqlite3");
    let recovery = tmp.path().join("recovery.sqlite3");
    assert!(migrate_legacy(&old, &native, &recovery).is_err());
    assert!(!native.exists());
    assert_eq!(std::fs::read(&old).unwrap(), before);
    assert_eq!(
        Connection::open(&recovery)
            .unwrap()
            .query_row("SELECT x FROM servers WHERE id='stable-b'", [], |r| r
                .get::<_, f64>(0))
            .unwrap(),
        10001.0
    );
    std::fs::write(&native, b"existing-owner-file").unwrap();
    assert!(migrate_legacy(&old, &native, &tmp.path().join("other-recovery")).is_err());
    assert_eq!(std::fs::read(&native).unwrap(), b"existing-owner-file");
    assert!(restore_legacy(&recovery, &native).is_err());
    assert_eq!(std::fs::read(&native).unwrap(), b"existing-owner-file");
}

#[test]
fn source_schema_extensions_are_rejected_without_executing_them() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old.sqlite3");
    legacy(&old);
    {
        let db = Connection::open(&old).unwrap();
        db.execute_batch(
            "CREATE TRIGGER unexpected AFTER UPDATE ON servers BEGIN DELETE FROM edges; END;",
        )
        .unwrap();
    }
    let native = tmp.path().join("native.sqlite3");
    let recovery = tmp.path().join("recovery.sqlite3");
    assert_eq!(
        migrate_legacy(&old, &native, &recovery).unwrap_err().code,
        ErrorCode::MigrationFailed
    );
    assert!(!native.exists());
    assert!(recovery.exists());
    assert_eq!(
        Connection::open(&old)
            .unwrap()
            .query_row("SELECT count(*) FROM edges", params![], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn sqlite_named_user_table_is_rejected_and_retained_in_recovery() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old.sqlite3");
    legacy(&old);
    {
        let db = Connection::open(&old).unwrap();
        db.execute_batch(
            "CREATE TABLE sqliteExtra(value TEXT); INSERT INTO sqliteExtra VALUES('owner data');",
        )
        .unwrap();
    }
    let native = tmp.path().join("native.sqlite3");
    let recovery = tmp.path().join("recovery.sqlite3");
    assert_eq!(
        migrate_legacy(&old, &native, &recovery).unwrap_err().code,
        ErrorCode::MigrationFailed
    );
    assert!(!native.exists());
    for path in [&old, &recovery] {
        let value: String = Connection::open(path)
            .unwrap()
            .query_row("SELECT value FROM sqliteExtra", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "owner data");
    }
}
