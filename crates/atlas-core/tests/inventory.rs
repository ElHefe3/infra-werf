use atlas_core::{
    inventory::{ImportRecord, MAX_REPORT_BYTES},
    *,
};
use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::{Value, json};

fn report(at: &str, host: &str) -> Value {
    json!({"hostname":host,"collected_at":at,"cpus":4,"memory_total_kib":4000000,
        "services":[{"scope":"system","name":"ssh.service","state":"active","substate":"running","boot":"enabled","restart":"always","raw":"excluded-marker"}],
        "containers":[{"scope":"user","engine":"podman","name":"web","image":"example/web:1","state":"running","restart":"always","env":"excluded-marker"}],
        "warnings":["excluded-marker"],"private_key":"excluded-marker","raw":"excluded-marker"})
}
fn input(key: &str, host: &str, at: &str) -> ImportRecord {
    ImportRecord {
        source_key: key.into(),
        default_name: "Imported".into(),
        report: report(at, host),
    }
}

#[test]
fn newer_import_preserves_identity_owner_edits_edges_and_excludes_persisted_raw_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("native.sqlite3");
    let mut store = Store::create(&path).unwrap();
    let id = store
        .add_server(AddServer {
            name: "Manual".into(),
            host: "observed.home".into(),
        })
        .unwrap();
    let other = store
        .add_server(AddServer {
            name: "Other".into(),
            host: "other.home".into(),
        })
        .unwrap();
    let edge = store
        .add_relationship(AddRelationship {
            source: id.clone(),
            target: other,
            label: "Uses".into(),
        })
        .unwrap();
    assert_eq!(
        store
            .import_batch(&[input("git", "observed.home", "2026-01-01T00:00:00Z")])
            .unwrap(),
        1
    );
    store
        .update_server(
            &id,
            UpdateServer {
                name: Some("Owner edited".into()),
                host: Some("edited.home".into()),
                x: Some(123.0),
                y: Some(456.0),
            },
        )
        .unwrap();
    assert_eq!(
        store
            .import_batch(&[input("git", "observed.home", "2026-01-02T00:00:00Z")])
            .unwrap(),
        1
    );
    let state = store.snapshot().unwrap();
    let server = state.servers.iter().find(|s| s.id == id).unwrap();
    assert_eq!(
        (&server.name, &server.host, server.x, server.y),
        (&"Owner edited".into(), &"edited.home".into(), 123.0, 456.0)
    );
    assert_eq!(
        server.inventory.as_ref().unwrap().collected_at,
        "2026-01-02T00:00:00Z"
    );
    assert_eq!(server.freshness, Freshness::Stale);
    assert_eq!(state.edges[0].id, edge);
    let events = state.events.len();
    for at in ["2026-01-02T02:00:00+02:00", "2026-01-01T00:00:00Z"] {
        assert_eq!(
            store
                .import_batch(&[input("git", "observed.home", at)])
                .unwrap(),
            0
        );
    }
    assert_eq!(store.snapshot().unwrap().events.len(), events);
    drop(store);
    let stored: String = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT report FROM servers WHERE id=?",
            [id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    for excluded in [
        "excluded-marker",
        "restart",
        "boot",
        "warnings",
        "private_key",
        "raw",
        "env",
    ] {
        assert!(!stored.contains(excluded));
    }
}

#[test]
fn bad_retained_fields_and_sizes_reject_the_entire_batch_before_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut store = Store::create(&tmp.path().join("native.sqlite3")).unwrap();
    let mut failures = vec![];
    for (field, value) in [
        ("cpus", json!(true)),
        ("memory_total_kib", json!(-1)),
        ("cpus", json!(1_000_000_000_000u64)),
        ("services", json!("wrong")),
        ("services", json!([{"name":""}])),
        ("containers", json!(vec![json!({"name":"x"}); 1025])),
        ("collected_at", json!("2999-01-01T00:00:00Z")),
    ] {
        let mut bad = report("2026-01-01T00:00:00Z", "bad.home");
        bad[field] = value;
        failures.push(bad);
    }
    let mut huge = report("2026-01-01T00:00:00Z", "bad.home");
    huge["unretained"] = json!("x".repeat(MAX_REPORT_BYTES));
    failures.push(huge);
    for bad in failures {
        let batch = [
            input("git", "good.home", "2026-01-01T00:00:00Z"),
            ImportRecord {
                source_key: "game".into(),
                default_name: "Bad".into(),
                report: bad,
            },
        ];
        assert!(store.import_batch(&batch).is_err());
        assert!(store.snapshot().unwrap().servers.is_empty());
        assert!(store.snapshot().unwrap().events.is_empty());
    }
}

#[test]
fn duplicate_or_conflicting_sources_cannot_adopt_another_imports_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let mut store = Store::create(&tmp.path().join("native.sqlite3")).unwrap();
    assert!(
        store
            .import_batch(&[
                input("git", "one.home", "2026-01-01T00:00:00Z"),
                input("git", "two.home", "2026-01-01T00:00:00Z")
            ])
            .is_err()
    );
    assert!(
        store
            .import_batch(&[
                input("git", "one.home", "2026-01-01T00:00:00Z"),
                input("game", "ONE.home", "2026-01-01T00:00:00Z")
            ])
            .is_err()
    );
    store
        .import_batch(&[input("git", "one.home", "2026-01-01T00:00:00Z")])
        .unwrap();
    let original = store.snapshot().unwrap().servers[0].id.clone();
    assert_eq!(
        store
            .import_batch(&[
                input("runner", "new.home", "2026-01-01T00:00:00Z"),
                input("game", "one.home", "2026-01-02T00:00:00Z")
            ])
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let after = store.snapshot().unwrap();
    assert_eq!(after.servers.len(), 1);
    assert_eq!(after.servers[0].id, original);
    assert_eq!(after.events.len(), 1);
}

#[test]
fn directory_loader_is_fixed_bounded_and_does_not_accept_arbitrary_files() {
    let tmp = tempfile::tempdir().unwrap();
    let directory = tmp.path().join("reports");
    std::fs::create_dir(&directory).unwrap();
    let mut store = Store::create(&tmp.path().join("native.sqlite3")).unwrap();
    std::fs::write(directory.join("unconfigured.json"), "not valid JSON").unwrap();
    assert_eq!(store.import_reports(&directory).unwrap(), 0);
    std::fs::write(
        directory.join("git.json"),
        serde_json::to_vec(&json!({"report":report("2026-01-01T00:00:00Z","one.home")})).unwrap(),
    )
    .unwrap();
    assert_eq!(store.import_reports(&directory).unwrap(), 1);
    assert_eq!(store.import_reports(&directory).unwrap(), 0);
    std::fs::write(directory.join("game.json"), b"invalid JSON").unwrap();
    assert!(store.import_reports(&directory).is_err());
    assert_eq!(store.snapshot().unwrap().servers.len(), 1);
    assert!(store.import_reports(&directory.join("missing")).is_err());
}

#[test]
fn authenticated_refresh_store_write_preserves_owner_map_fields_and_rejects_old_source_times() {
    let tmp = tempfile::tempdir().unwrap();
    let mut store = Store::create(&tmp.path().join("native.sqlite3")).unwrap();
    let id = store
        .add_server(AddServer {
            name: "Owner name".into(),
            host: "owner-ssh-alias.home".into(),
        })
        .unwrap();
    let other = store
        .add_server(AddServer {
            name: "Other host".into(),
            host: "other-ssh-alias.home".into(),
        })
        .unwrap();
    let edge = store
        .add_relationship(AddRelationship {
            source: id.clone(),
            target: other,
            label: "owner link".into(),
        })
        .unwrap();
    store
        .update_server(
            &id,
            UpdateServer {
                name: Some("Renamed by owner".into()),
                host: Some("renamed-ssh-alias.home".into()),
                x: Some(321.0),
                y: Some(654.0),
            },
        )
        .unwrap();
    let before = store.snapshot().unwrap();
    let now = Utc::now();
    let collected_at =
        (now - chrono::Duration::seconds(2)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let inventory = BaselineInventory {
        hostname: "fixture-node.home".into(),
        collected_at: collected_at.clone(),
        source_host: None,
        source_binding: None,
        cpus: 4,
        memory_total_kib: 4_000_000,
        services: vec![ServiceSummary {
            scope: "system".into(),
            name: "ssh.service".into(),
            state: "active".into(),
            substate: "running".into(),
        }],
        containers: vec![ContainerSummary {
            scope: "user".into(),
            engine: "podman".into(),
            name: "api".into(),
            image: "localhost/api:v1".into(),
            state: "running".into(),
        }],
    };
    let source_binding = format!("src1_{}", "a".repeat(64));
    assert!(
        store
            .refresh_inventory(&id, &inventory, &source_binding)
            .unwrap()
    );
    let after = store.snapshot().unwrap();
    assert_eq!(after.edges.len(), 1);
    assert_eq!(after.edges[0].id, edge);
    let before_server = before
        .servers
        .iter()
        .find(|server| server.id == id)
        .unwrap();
    let after_server = after.servers.iter().find(|server| server.id == id).unwrap();
    assert_eq!(
        after_server
            .inventory
            .as_ref()
            .unwrap()
            .source_host
            .as_deref(),
        Some("renamed-ssh-alias.home")
    );
    assert_eq!(
        after_server
            .inventory
            .as_ref()
            .unwrap()
            .source_binding
            .as_deref(),
        Some(source_binding.as_str())
    );
    assert_eq!(after_server.name, before_server.name);
    assert_eq!(after_server.host, before_server.host);
    assert_eq!(
        (after_server.x, after_server.y),
        (before_server.x, before_server.y)
    );
    assert_eq!(after_server.created_at, before_server.created_at);
    assert_eq!(
        after_server.inventory.as_ref().unwrap().collected_at,
        collected_at
    );
    assert!(
        !store
            .refresh_inventory(&id, &inventory, &source_binding)
            .unwrap()
    );

    let mut older = inventory;
    older.collected_at =
        (now - chrono::Duration::minutes(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    assert!(
        !store
            .refresh_inventory(&id, &older, &source_binding)
            .unwrap()
    );
    let final_server = store
        .snapshot()
        .unwrap()
        .servers
        .into_iter()
        .find(|server| server.id == id)
        .unwrap();
    assert_eq!(final_server.inventory.unwrap().collected_at, collected_at);
}

#[test]
fn equal_time_refresh_rebinds_only_an_identical_imported_inventory() {
    let collected_at = "2026-01-01T00:00:00Z";
    for identical in [true, false] {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = Store::create(&tmp.path().join("native.sqlite3")).unwrap();
        let id = store
            .add_server(AddServer {
                name: "Fixture host".into(),
                host: "fixture-node.home".into(),
            })
            .unwrap();
        assert_eq!(
            store
                .import_batch(&[input("git", "fixture-node.home", collected_at)])
                .unwrap(),
            1
        );
        store
            .update_server(
                &id,
                UpdateServer {
                    host: Some("fixture-ssh-alias.home".into()),
                    ..UpdateServer::default()
                },
            )
            .unwrap();
        let imported = store.snapshot().unwrap();
        let event_count = imported.events.len();
        let mut incoming =
            atlas_core::inventory::classify(&report(collected_at, "fixture-node.home"), Utc::now())
                .unwrap();
        if !identical {
            incoming.containers.clear();
        }

        let source_binding = format!("src1_{}", if identical { "b" } else { "c" }.repeat(64));
        assert!(
            !store
                .refresh_inventory(&id, &incoming, &source_binding)
                .unwrap()
        );
        let after = store.snapshot().unwrap();
        let saved = after
            .servers
            .iter()
            .find(|server| server.id == id)
            .unwrap()
            .inventory
            .as_ref()
            .unwrap();
        assert_eq!(
            saved.source_host.as_deref(),
            if identical {
                Some("fixture-ssh-alias.home")
            } else {
                None
            }
        );
        assert_eq!(
            saved.source_binding.as_deref(),
            if identical {
                Some(source_binding.as_str())
            } else {
                None
            }
        );
        assert_eq!(saved.containers.len(), 1);
        assert_eq!(after.events.len(), event_count);
    }
}
