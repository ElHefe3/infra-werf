use super::*;
use atlas_core::{monitoring::*, ssh::SshCheckState};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct LiveScheduler {
    next_tick: Option<Instant>,
    next_discovery: Option<Instant>,
    pub(super) running: HashSet<ServerId>,
    due: HashMap<ServerId, (String, Instant)>,
    pub(super) discovery_running: bool,
}
impl Workspace {
    pub fn accept_live_identity(
        self: &Arc<Self>,
        id: &ServerId,
        previous: &str,
        accepted: &str,
    ) -> Result<MonitoringState, AtlasError> {
        let _contact = self.live_contact_guard()?;
        let record = self
            .monitoring_state()?
            .records
            .into_iter()
            .find(|r| {
                r.server_id == *id
                    && r.fingerprint.as_deref() == Some(previous)
                    && r.pending_fingerprint.as_deref() == Some(accepted)
                    && r.policy.permits_contact()
            })
            .ok_or_else(|| AtlasError::new(ErrorCode::Conflict))?;
        if !self.live_current(&record) {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        let ticket = self.prepare_ssh(id)?;
        let check = self.verify_ssh_inner(id, &ticket.operation_id)?;
        if check.state != SshCheckState::Verified || check.fingerprint.as_deref() != Some(accepted)
        {
            return Err(AtlasError::new(ErrorCode::HostTrustFailure));
        }
        self.with_store(|s| s.accept_monitoring_identity(id, previous, accepted))?;
        self.monitoring_state()
    }
    pub fn monitoring_state(&self) -> Result<MonitoringState, AtlasError> {
        self.with_store(|s| s.monitoring())
    }
    pub fn set_monitoring(
        &self,
        id: &ServerId,
        policy: MonitoringPolicy,
    ) -> Result<MonitoringState, AtlasError> {
        let _contact = self.live_contact_guard()?;
        self.with_store(|s| s.set_monitoring(id, policy))?;
        self.cancel_health(id)?;
        self.invalidate_server(id);
        self.monitoring_state()
    }
    pub fn history_limit(&self, limit: u32) -> Result<MonitoringState, AtlasError> {
        self.with_store(|s| s.set_history_limit(limit))?;
        self.monitoring_state()
    }
    pub fn discover_configured(&self) -> Result<MonitoringState, AtlasError> {
        let aliases = self.ssh.configured_aliases()?;
        self.with_store(|s| {
            let state = s.snapshot()?;
            let mut known: HashSet<String> = state
                .servers
                .iter()
                .map(|s| s.host.to_ascii_lowercase())
                .collect();
            for alias in aliases {
                if known.insert(alias.to_ascii_lowercase()) {
                    s.add_monitoring_candidate(&alias)?;
                }
            }
            s.monitoring()
        })
    }
    fn live_current(&self, record: &MonitoringRecord) -> bool {
        self.monitoring_state().is_ok_and(|s| {
            s.records.iter().any(|r| {
                r.server_id == record.server_id
                    && r.host == record.host
                    && r.revision == record.revision
                    && r.policy.permits_contact()
            })
        }) && self
            .ssh_target(&record.server_id)
            .is_ok_and(|t| t.host() == record.host)
    }
    pub fn tick_live(self: &Arc<Self>) {
        let Ok(mut scheduler) = self.live.lock() else {
            return;
        };
        let now = Instant::now();
        if scheduler.next_tick.is_some_and(|due| due > now) {
            return;
        }
        scheduler.next_tick = Some(now + Duration::from_secs(1));
        if !scheduler.discovery_running && scheduler.next_discovery.is_none_or(|due| due <= now) {
            scheduler.discovery_running = true;
            scheduler.next_discovery = Some(now + Duration::from_secs(60));
            let workspace = self.clone();
            if thread::Builder::new()
                .name("infra-config-discovery".into())
                .spawn(move || {
                    if let Ok(_operation) = workspace.begin() {
                        let _ = workspace.discover_configured();
                    }
                    if let Ok(mut scheduler) = workspace.live.lock() {
                        scheduler.discovery_running = false;
                    }
                })
                .is_err()
            {
                scheduler.discovery_running = false;
            }
        }
        // One remote worker at a time; no overlap per server, even after policy edits.
        if !scheduler.running.is_empty() {
            return;
        }
        let Ok(state) = self.monitoring_state() else {
            return;
        };
        let known: HashSet<_> = state.records.iter().map(|r| r.server_id.clone()).collect();
        scheduler.due.retain(|id, _| known.contains(id));
        let selected = state
            .records
            .into_iter()
            .filter(|r| {
                r.policy.permits_contact()
                    && !scheduler.running.contains(&r.server_id)
                    && scheduler
                        .due
                        .get(&r.server_id)
                        .is_none_or(|(revision, due)| revision != &r.revision || *due <= now)
            })
            .min_by_key(|r| {
                scheduler
                    .due
                    .get(&r.server_id)
                    .filter(|(revision, _)| revision == &r.revision)
                    .map(|(_, due)| *due)
            });
        if let Some(record) = selected {
            let force_inventory = scheduler
                .due
                .get(&record.server_id)
                .is_none_or(|(revision, _)| revision != &record.revision);
            let Ok(operation) = self.begin() else { return };
            scheduler.running.insert(record.server_id.clone());
            let workspace = self.clone();
            let failed_id = record.server_id.clone();
            if thread::Builder::new().name("infra-live-check".into()).spawn(move || {
                let _operation=operation;
                let mut observation=record.clone();
                let contact=workspace.live_contact_guard();
                if contact.is_ok() && workspace.live_current(&record) {
                    let checked=workspace.prepare_ssh(&record.server_id).and_then(|ticket|workspace.verify_ssh_inner(&record.server_id,&ticket.operation_id));
                    let at=chrono::Utc::now().to_rfc3339();
                    let mut verified=false;
                    let mut source_binding=None;
                    let (state,detail)=match checked {
                        Ok(check) => {
                            let changed=record.fingerprint.is_some() && check.fingerprint.is_some() && record.fingerprint!=check.fingerprint;
                            if changed { observation.pending_fingerprint=check.fingerprint; (Reachability::Reachable,"SSH identity changed; review required") }
                            else { match check.state {
                                SshCheckState::Verified=>{verified=true; observation.pending_fingerprint=None; source_binding=check.profile.as_ref().zip(check.fingerprint.as_deref()).map(|(profile,key)|atlas_core::inventory::authenticated_source_binding(profile,key)); observation.fingerprint=check.fingerprint; (Reachability::Reachable,"Reachable · SSH verified")},
                                SshCheckState::Unreachable=>(Reachability::Unreachable,"Unreachable from this PC"),
                                SshCheckState::AuthenticationFailed=>(Reachability::Reachable,"Reachable · SSH authentication failed"),
                                SshCheckState::UnknownHost|SshCheckState::ChangedHost=>(Reachability::Reachable,"Reachable · SSH trust review required"),
                                SshCheckState::UnsupportedConfiguration=>(Reachability::Unknown,"SSH configuration unsupported"),
                                SshCheckState::ClientUnavailable=>(Reachability::Unknown,"SSH client unavailable"),
                                SshCheckState::Failed=>(Reachability::Unknown,"SSH check inconclusive"),
                            }}
                        },
                        Err(_)=>(Reachability::Unknown,"Check incomplete; saved inventory retained"),
                    };
                    observation.detail=detail.into();
                    observation.reachability.state=state;
                    observation.reachability.last_checked_at=Some(at.clone());
                    match state { Reachability::Reachable=>observation.reachability.last_success_at=Some(at.clone()), Reachability::Unreachable=>observation.reachability.last_failure_at=Some(at.clone()), _=>{} }
                    let inventory_due=force_inventory || record.inventory_checked_at.as_deref().and_then(|at|chrono::DateTime::parse_from_rfc3339(at).ok()).is_none_or(|at|(chrono::Utc::now()-at.with_timezone(&chrono::Utc)).num_seconds()>=i64::from(record.policy.inventory_interval_seconds));
                    if verified && record.policy.inventory_enabled && inventory_due && workspace.live_current(&record) {
                        let refreshed=workspace.refresh_inventory_guarded(&record.server_id,Some(&record.revision),source_binding.as_deref());
                        observation.inventory_checked_at=Some(at);
                        observation.inventory_detail=Some(match refreshed { Ok(result) if result.status!=InventoryRefreshStatus::Partial=>"Automatic inventory collection complete", Ok(_)=>"Inventory collection partial; previous inventory retained", Err(_)=>"Automatic inventory collection failed; previous inventory retained" }.into());
                    }
                    let _=workspace.with_store(|s|s.observe_monitoring(&observation));
                }
                drop(contact);
                if let Ok(mut scheduler)=workspace.live.lock() {
                    scheduler.running.remove(&record.server_id);
                    scheduler.due.insert(record.server_id,(record.revision,Instant::now()+Duration::from_secs(u64::from(record.policy.interval_seconds))));
                }
            }).is_err() {scheduler.running.remove(&failed_id);}
        }
    }
}
