//! Persistent local monitoring policy. Observations never grant SSH authority.
use crate::{
    store::{event, sql_error},
    *,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

pub(crate) const SCHEMA: &str = "
CREATE TABLE monitoring (server_id TEXT PRIMARY KEY REFERENCES servers(id) ON DELETE CASCADE, payload TEXT NOT NULL);
CREATE TABLE monitoring_settings (id INTEGER PRIMARY KEY CHECK(id=1), history_limit INTEGER NOT NULL);
INSERT INTO monitoring_settings VALUES(1,1000);
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerLifecycle {
    Candidate,
    #[default]
    Active,
    Archived,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MonitoringPolicy {
    pub lifecycle: ServerLifecycle,
    pub enabled: bool,
    pub owner_offline: bool,
    pub inventory_enabled: bool,
    pub interval_seconds: u32,
    pub inventory_interval_seconds: u32,
}
impl Default for MonitoringPolicy {
    fn default() -> Self {
        Self {
            lifecycle: ServerLifecycle::Active,
            enabled: false,
            owner_offline: false,
            inventory_enabled: false,
            interval_seconds: 60,
            inventory_interval_seconds: 900,
        }
    }
}
impl MonitoringPolicy {
    pub fn permits_contact(&self) -> bool {
        self.lifecycle == ServerLifecycle::Active && self.enabled && !self.owner_offline
    }
    pub fn validate(&self) -> Result<(), AtlasError> {
        if !(30..=3600).contains(&self.interval_seconds)
            || !(300..=86400).contains(&self.inventory_interval_seconds)
        {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MonitoringRecord {
    pub server_id: ServerId,
    pub host: String,
    pub policy: MonitoringPolicy,
    pub reachability: HostReachability,
    pub detail: String,
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub pending_fingerprint: Option<String>,
    pub inventory_checked_at: Option<String>,
    pub inventory_detail: Option<String>,
    pub revision: String,
}
impl MonitoringRecord {
    pub fn new(id: ServerId, host: String) -> Self {
        Self {
            server_id: id,
            host,
            policy: MonitoringPolicy::default(),
            reachability: HostReachability::default(),
            detail: "Not monitored".into(),
            fingerprint: None,
            pending_fingerprint: None,
            inventory_checked_at: None,
            inventory_detail: None,
            revision: ServerId::generate().as_str().to_owned(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MonitoringState {
    pub contract_version: u32,
    pub records: Vec<MonitoringRecord>,
    pub history_limit: u32,
}

impl Store {
    pub fn accept_monitoring_identity(
        &mut self,
        id: &ServerId,
        previous: &str,
        accepted: &str,
    ) -> Result<(), AtlasError> {
        let mut record = self
            .monitoring()?
            .records
            .into_iter()
            .find(|r| {
                r.server_id == *id
                    && r.fingerprint.as_deref() == Some(previous)
                    && r.pending_fingerprint.as_deref() == Some(accepted)
            })
            .ok_or_else(|| AtlasError::new(ErrorCode::Conflict))?;
        record.fingerprint = Some(accepted.to_owned());
        record.pending_fingerprint = None;
        record.revision = ServerId::generate().as_str().to_owned();
        record.detail = "Reviewed SSH identity accepted; waiting for check".into();
        record.reachability = HostReachability::default();
        let tx = self.db.transaction().map_err(sql_error)?;
        write_record(&tx, &record)?;
        event(
            &tx,
            "Owner accepted reviewed replacement SSH identity",
            Some(id.as_str()),
        )?;
        tx.commit().map_err(sql_error)
    }
    pub fn add_monitoring_candidate(&mut self, alias: &str) -> Result<ServerId, AtlasError> {
        let host = hostname(alias)?;
        let id = ServerId::generate();
        let tx = self.db.transaction().map_err(sql_error)?;
        let count: i64 = tx
            .query_row("SELECT count(*) FROM servers", [], |r| r.get(0))
            .map_err(sql_error)?;
        tx.execute(
            "INSERT INTO servers VALUES(?,?,?,?,?,?,NULL,NULL)",
            params![
                id.as_str(),
                host.chars().take(80).collect::<String>(),
                host,
                45.0 + (count % 3) as f64 * 300.0,
                65.0 + (count / 3) as f64 * 310.0,
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .map_err(sql_error)?;
        let mut record = MonitoringRecord::new(id.clone(), host);
        record.policy.lifecycle = ServerLifecycle::Candidate;
        record.detail = "Discovered from primary SSH configuration; awaiting review".into();
        write_record(&tx, &record)?;
        event(
            &tx,
            "Candidate discovered from primary SSH configuration",
            Some(id.as_str()),
        )?;
        tx.commit().map_err(sql_error)?;
        Ok(id)
    }
    pub fn monitoring(&self) -> Result<MonitoringState, AtlasError> {
        let mut records = Vec::new();
        let mut query = self
            .db
            .prepare("SELECT payload FROM monitoring ORDER BY server_id")
            .map_err(sql_error)?;
        let rows = query
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(sql_error)?;
        for row in rows {
            records.push(
                serde_json::from_str(&row.map_err(sql_error)?)
                    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?,
            );
        }
        let history_limit = self
            .db
            .query_row(
                "SELECT history_limit FROM monitoring_settings WHERE id=1",
                [],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        Ok(MonitoringState {
            contract_version: CONTRACT_VERSION,
            records,
            history_limit,
        })
    }
    pub fn set_monitoring(
        &mut self,
        id: &ServerId,
        policy: MonitoringPolicy,
    ) -> Result<(), AtlasError> {
        policy.validate()?;
        let host: String = self
            .db
            .query_row("SELECT host FROM servers WHERE id=?", [id.as_str()], |r| {
                r.get(0)
            })
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let mut record = self
            .monitoring()?
            .records
            .into_iter()
            .find(|r| r.server_id == *id)
            .unwrap_or_else(|| MonitoringRecord::new(id.clone(), host.clone()));
        record.host = host;
        record.policy = policy;
        record.revision = ServerId::generate().as_str().to_owned();
        record.reachability = HostReachability::default();
        record.detail = if record.policy.owner_offline {
            "Owner-reported offline; checks paused"
        } else if record.policy.permits_contact() {
            "Waiting for check"
        } else {
            "Checks paused"
        }
        .into();
        let tx = self.db.transaction().map_err(sql_error)?;
        write_record(&tx, &record)?;
        event(
            &tx,
            &format!(
                "Monitoring policy: {:?}{}",
                record.policy.lifecycle,
                if record.policy.owner_offline {
                    "; owner-reported offline"
                } else if record.policy.permits_contact() {
                    "; enabled"
                } else {
                    "; paused"
                }
            ),
            Some(id.as_str()),
        )?;
        tx.commit().map_err(sql_error)
    }
    /// Compare-and-set prevents an in-flight check restoring deleted/paused/edited policy.
    pub fn observe_monitoring(&mut self, record: &MonitoringRecord) -> Result<bool, AtlasError> {
        let current = self.monitoring()?.records.into_iter().find(|r| {
            r.server_id == record.server_id
                && r.revision == record.revision
                && r.host == record.host
                && r.policy.permits_contact()
        });
        let Some(current) = current else {
            return Ok(false);
        };
        let host: Option<String> = self
            .db
            .query_row(
                "SELECT host FROM servers WHERE id=?",
                [record.server_id.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        if host.as_deref() != Some(record.host.as_str()) {
            return Ok(false);
        }
        let tx = self.db.transaction().map_err(sql_error)?;
        if current.detail != record.detail
            || current.reachability.state != record.reachability.state
        {
            event(&tx, &record.detail, Some(record.server_id.as_str()))?;
        }
        if current.inventory_detail != record.inventory_detail
            && let Some(detail) = &record.inventory_detail
        {
            event(&tx, detail, Some(record.server_id.as_str()))?;
        }
        write_record(&tx, record)?;
        tx.commit().map_err(sql_error)?;
        Ok(true)
    }
    pub fn set_history_limit(&mut self, limit: u32) -> Result<(), AtlasError> {
        if !(200..=10000).contains(&limit) {
            return Err(AtlasError::new(ErrorCode::InvalidInput));
        }
        let tx = self.db.transaction().map_err(sql_error)?;
        tx.execute(
            "UPDATE monitoring_settings SET history_limit=? WHERE id=1",
            [limit],
        )
        .map_err(sql_error)?;
        event(&tx, "History retention updated", None)?;
        tx.commit().map_err(sql_error)
    }
}
/// The discovered candidate holding `host`, if any. A host held by an active,
/// archived or ignored server is not a candidate. `except` skips one server.
fn candidate_for_host(
    db: &rusqlite::Connection,
    host: &str,
    except: Option<&ServerId>,
) -> Result<Option<MonitoringRecord>, AtlasError> {
    let payload: Option<String> = db
        .query_row(
            "SELECT m.payload FROM servers s JOIN monitoring m ON m.server_id=s.id WHERE s.host=?1 AND s.id<>?2",
            params![host, except.map_or("", |id| id.as_str())],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let record: MonitoringRecord =
        serde_json::from_str(&payload).map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
    Ok((record.policy.lifecycle == ServerLifecycle::Candidate).then_some(record))
}
/// Adopts a server that exists only as a discovered candidate: the owner is
/// adding a host InfraWerf already found in the SSH configuration. The entry
/// takes the owner's name and becomes active; its ID, position, relationships
/// and history are kept and checks stay off until enabled. Returns `None` when
/// the host is not a candidate, so any other duplicate host still conflicts.
pub(crate) fn adopt_candidate(
    db: &rusqlite::Connection,
    host: &str,
    name: &str,
) -> Result<Option<ServerId>, AtlasError> {
    let Some(mut record) = candidate_for_host(db, host, None)? else {
        return Ok(None);
    };
    let id = record.server_id.clone();
    record.policy.lifecycle = ServerLifecycle::Active;
    record.revision = ServerId::generate().as_str().to_owned();
    record.reachability = HostReachability::default();
    record.detail = "Checks paused".into();
    db.execute(
        "UPDATE servers SET name=?1 WHERE id=?2",
        params![name, id.as_str()],
    )
    .map_err(sql_error)?;
    write_record(db, &record)?;
    event(
        db,
        &format!("Server added: {name} (discovered candidate adopted)"),
        Some(id.as_str()),
    )?;
    Ok(Some(id))
}
/// Frees a host held only by a discovered candidate so the owner can give it
/// to another server, and returns the removed candidate's ID. A candidate with
/// documented relationships is never discarded this way: the function returns
/// `None` and the caller's update conflicts, so the owner decides what to do
/// with it. Also `None` when no other server holds the host.
pub(crate) fn release_candidate_host(
    db: &rusqlite::Connection,
    host: &str,
    keep: &ServerId,
) -> Result<Option<ServerId>, AtlasError> {
    let Some(record) = candidate_for_host(db, host, Some(keep))? else {
        return Ok(None);
    };
    let id = record.server_id;
    let relationships: i64 = db
        .query_row(
            "SELECT count(*) FROM edges WHERE source=?1 OR target=?1",
            [id.as_str()],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if relationships > 0 {
        return Ok(None);
    }
    crate::store::delete_server_rows(db, &id)?;
    event(
        db,
        &format!(
            "Discovered candidate {} replaced by this server's new address",
            record.host
        ),
        Some(keep.as_str()),
    )?;
    Ok(Some(id))
}
fn write_record(db: &rusqlite::Connection, record: &MonitoringRecord) -> Result<(), AtlasError> {
    let json = serde_json::to_string(record).map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
    db.execute("INSERT INTO monitoring VALUES(?,?) ON CONFLICT(server_id) DO UPDATE SET payload=excluded.payload",params![record.server_id.as_str(),json]).map_err(sql_error)?;
    Ok(())
}
pub fn typescript_contracts(config: &Config) -> String {
    [
        ServerLifecycle::decl(config),
        MonitoringPolicy::decl(config),
        MonitoringRecord::decl(config),
        MonitoringState::decl(config),
    ]
    .into_iter()
    .map(|d| format!("export {d}\n"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adding_a_discovered_candidate_adopts_it_instead_of_conflicting() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("map.db")).unwrap();
        let candidate = store.add_monitoring_candidate("lab-alias").unwrap();
        let before = store.snapshot().unwrap().servers[0].clone();
        // Host matching is case-insensitive, like the unique host rule.
        let adopted = store
            .add_server(AddServer {
                name: "Lab server".into(),
                host: "LAB-ALIAS".into(),
            })
            .unwrap();
        assert_eq!(adopted, candidate);
        let state = store.snapshot().unwrap();
        assert_eq!(state.servers.len(), 1);
        let server = &state.servers[0];
        assert_eq!(server.name, "Lab server");
        assert_eq!((server.x, server.y), (before.x, before.y));
        assert_eq!(server.host, "lab-alias");
        let record = store.monitoring().unwrap().records.remove(0);
        assert_eq!(record.policy.lifecycle, ServerLifecycle::Active);
        assert!(!record.policy.enabled && !record.policy.permits_contact());
        assert!(
            state
                .events
                .iter()
                .any(|e| e.message.contains("discovered candidate adopted"))
        );
        // Once active it is an ordinary server: adding it again conflicts.
        assert_eq!(
            store
                .add_server(AddServer {
                    name: "Again".into(),
                    host: "lab-alias".into(),
                })
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
    }

    #[test]
    fn editing_a_host_to_a_discovered_candidate_replaces_the_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("map.db")).unwrap();
        let server = store
            .add_server(AddServer {
                name: "Lab".into(),
                host: "lab-old".into(),
            })
            .unwrap();
        let candidate = store.add_monitoring_candidate("lab-new").unwrap();
        let host_only = |host: &str| UpdateServer {
            name: None,
            host: Some(host.into()),
            x: None,
            y: None,
        };
        assert_eq!(
            store
                .update_server_replacing(&server, host_only("LAB-NEW"))
                .unwrap(),
            Some(candidate.clone())
        );
        let state = store.snapshot().unwrap();
        assert_eq!(state.servers.len(), 1);
        assert_eq!(state.servers[0].id, server);
        assert_eq!(state.servers[0].host, "LAB-NEW");
        assert!(
            store
                .monitoring()
                .unwrap()
                .records
                .iter()
                .all(|r| r.server_id != candidate)
        );
        assert!(
            state
                .events
                .iter()
                .all(|e| e.server_id.as_ref() != Some(&candidate))
        );
        assert!(
            state
                .events
                .iter()
                .any(|e| e.message.contains("candidate lab-new replaced"))
        );
        // A candidate with a documented relationship is not discarded: the
        // edit conflicts and the candidate and its relationship remain.
        let linked = store.add_monitoring_candidate("lab-linked").unwrap();
        store
            .add_relationship(AddRelationship {
                kind: None,
                source: server.clone(),
                target: linked.clone(),
                label: "Backs up to".into(),
            })
            .unwrap();
        assert_eq!(
            store
                .update_server(&server, host_only("lab-linked"))
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
        let kept = store.snapshot().unwrap();
        assert!(kept.servers.iter().any(|s| s.id == linked));
        assert_eq!(kept.edges.len(), 1);
        store.remove_server(&linked).unwrap();
        // A host held by a reviewed (active) server still conflicts, and the
        // failed edit changes nothing.
        let other = store.add_monitoring_candidate("lab-other").unwrap();
        store
            .set_monitoring(&other, MonitoringPolicy::default())
            .unwrap();
        assert_eq!(
            store
                .update_server(&server, host_only("lab-other"))
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
        let state = store.snapshot().unwrap();
        assert_eq!(state.servers.len(), 2);
        assert!(
            state
                .servers
                .iter()
                .any(|s| s.id == server && s.host == "LAB-NEW")
        );
    }

    #[test]
    fn archived_ignored_and_plain_hosts_still_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("map.db")).unwrap();
        for (host, lifecycle) in [
            ("archived-alias", ServerLifecycle::Archived),
            ("ignored-alias", ServerLifecycle::Ignored),
        ] {
            let id = store.add_monitoring_candidate(host).unwrap();
            store
                .set_monitoring(
                    &id,
                    MonitoringPolicy {
                        lifecycle,
                        ..MonitoringPolicy::default()
                    },
                )
                .unwrap();
        }
        store
            .add_server(AddServer {
                name: "Plain".into(),
                host: "plain-host".into(),
            })
            .unwrap();
        for host in ["archived-alias", "ignored-alias", "plain-host"] {
            assert_eq!(
                store
                    .add_server(AddServer {
                        name: "Duplicate".into(),
                        host: host.into(),
                    })
                    .unwrap_err()
                    .code,
                ErrorCode::Conflict,
                "{host}"
            );
        }
        assert_eq!(store.snapshot().unwrap().servers.len(), 3);
    }

    #[test]
    fn failed_candidate_policy_write_rolls_back_entire_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("map.db")).unwrap();
        store.db.execute_batch("CREATE TRIGGER fixture_failure BEFORE INSERT ON monitoring BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        assert!(store.add_monitoring_candidate("candidate.invalid").is_err());
        assert!(store.snapshot().unwrap().servers.is_empty());
        assert!(store.snapshot().unwrap().events.is_empty());
        store
            .db
            .execute_batch("DROP TRIGGER fixture_failure;")
            .unwrap();
        let id = store.add_monitoring_candidate("candidate.invalid").unwrap();
        assert_eq!(store.monitoring().unwrap().records[0].server_id, id);
        assert_eq!(
            store.monitoring().unwrap().records[0].policy.lifecycle,
            ServerLifecycle::Candidate
        );
        let long_alias = format!("{}.{}.invalid", "a".repeat(60), "b".repeat(60));
        let long_id = store.add_monitoring_candidate(&long_alias).unwrap();
        let server = store
            .snapshot()
            .unwrap()
            .servers
            .into_iter()
            .find(|s| s.id == long_id)
            .unwrap();
        assert_eq!(server.name.len(), 80);
        assert_eq!(server.host, long_alias);
    }
    #[test]
    fn replacement_identity_requires_matching_pending_key_and_retains_pin_on_host_edit() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("map.db")).unwrap();
        let id = add(&mut store);
        let policy = MonitoringPolicy {
            enabled: true,
            ..Default::default()
        };
        store.set_monitoring(&id, policy.clone()).unwrap();
        let mut record = store.monitoring().unwrap().records.remove(0);
        record.fingerprint = Some("old".into());
        record.pending_fingerprint = Some("new".into());
        store.observe_monitoring(&record).unwrap();
        assert!(
            store
                .accept_monitoring_identity(&id, "wrong", "new")
                .is_err()
        );
        assert!(
            store
                .accept_monitoring_identity(&id, "old", "wrong")
                .is_err()
        );
        store.accept_monitoring_identity(&id, "old", "new").unwrap();
        assert_eq!(
            store.monitoring().unwrap().records[0]
                .fingerprint
                .as_deref(),
            Some("new")
        );
        store
            .update_server(
                &id,
                UpdateServer {
                    host: Some("replacement.invalid".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        store.set_monitoring(&id, policy).unwrap();
        assert_eq!(
            store.monitoring().unwrap().records[0]
                .fingerprint
                .as_deref(),
            Some("new")
        );
    }
    fn add(store: &mut Store) -> ServerId {
        store
            .add_server(AddServer {
                name: "Node".into(),
                host: "node.invalid".into(),
            })
            .unwrap()
    }
    #[test]
    fn observations_are_transition_only_and_paused_revisions_cannot_return() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("map.db");
        let mut store = Store::create(&path).unwrap();
        let id = add(&mut store);
        let policy = MonitoringPolicy {
            enabled: true,
            ..Default::default()
        };
        store.set_monitoring(&id, policy.clone()).unwrap();
        let mut record = store.monitoring().unwrap().records.remove(0);
        record.reachability.state = Reachability::Unreachable;
        record.detail = "Unreachable from this PC".into();
        assert!(store.observe_monitoring(&record).unwrap());
        let count = store.snapshot().unwrap().events.len();
        assert!(store.observe_monitoring(&record).unwrap());
        assert_eq!(store.snapshot().unwrap().events.len(), count);
        record.reachability.state = Reachability::Reachable;
        record.detail = "Reachable · SSH verified".into();
        assert!(store.observe_monitoring(&record).unwrap());
        for lifecycle in [
            ServerLifecycle::Archived,
            ServerLifecycle::Ignored,
            ServerLifecycle::Candidate,
        ] {
            store
                .set_monitoring(
                    &id,
                    MonitoringPolicy {
                        lifecycle,
                        ..policy.clone()
                    },
                )
                .unwrap();
            assert!(!store.observe_monitoring(&record).unwrap());
            assert!(
                !store.monitoring().unwrap().records[0]
                    .policy
                    .permits_contact()
            );
        }
        store
            .set_monitoring(
                &id,
                MonitoringPolicy {
                    owner_offline: true,
                    ..policy
                },
            )
            .unwrap();
        assert!(!store.observe_monitoring(&record).unwrap());
        drop(store);
        let mut store = Store::open(&path).unwrap();
        assert!(store.monitoring().unwrap().records[0].policy.owner_offline);
        store.remove_server(&id).unwrap();
        assert!(store.monitoring().unwrap().records.is_empty());
        assert!(
            store
                .snapshot()
                .unwrap()
                .events
                .iter()
                .all(|e| e.server_id.as_ref() != Some(&id))
        );
    }
    #[test]
    fn v2_upgrade_keeps_a_complete_recovery_copy_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("map.db");
        let mut store = Store::create(&path).unwrap();
        let id = add(&mut store);
        store
            .db
            .execute_batch(
                "DROP TABLE monitoring; DROP TABLE monitoring_settings; PRAGMA user_version=2;",
            )
            .unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.snapshot().unwrap().servers[0].id, id);
        assert!(store.monitoring().unwrap().records.is_empty());
        drop(store);
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("pre-monitoring-")
            })
            .collect();
        assert_eq!(backups.len(), 1);
        let recovery = rusqlite::Connection::open(&backups[0]).unwrap();
        assert_eq!(
            recovery
                .pragma_query_value::<i32, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            2
        );
        assert_eq!(
            recovery
                .query_row("SELECT id FROM servers", [], |r| r.get::<_, String>(0))
                .unwrap(),
            id.as_str()
        );
        assert_eq!(
            recovery
                .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        drop(recovery);
        drop(Store::open(&path).unwrap());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    #[test]
    fn history_retention_is_bounded_and_endpoint_edits_reject_old_observations() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("map.db")).unwrap();
        let id = add(&mut store);
        store
            .set_monitoring(
                &id,
                MonitoringPolicy {
                    enabled: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let record = store.monitoring().unwrap().records.remove(0);
        store
            .update_server(
                &id,
                UpdateServer {
                    host: Some("changed.invalid".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!store.observe_monitoring(&record).unwrap());
        assert!(store.set_history_limit(199).is_err());
        store.set_history_limit(200).unwrap();
        for _ in 0..220 {
            event(&store.db, "fixture transition", Some(id.as_str())).unwrap();
        }
        assert_eq!(store.snapshot().unwrap().events.len(), 200);
    }
}
