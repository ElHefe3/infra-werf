//! Pack commands owned by the workspace (F21/T043).
//!
//! A run is admitted by the owner's settings in the store, goes through a
//! short-lived verified session like an inventory refresh, and leaves one
//! observation in memory. Observations are never written to disk and are
//! dropped whenever what plugins may do changes.
use super::*;
use atlas_core::pack::{
    PackAppsState, PackBadgeView, PackCommandView, PackMapState, PackObservation, PackOutcome,
    PackParameterView, PackResult, PackServerState,
};
use atlas_core::plugin_host::{ActivityOutcome, PackRun};
use atlas_core::tunnel::{TunnelInfo, TunnelRequest};
use atlas_core::workloads::WorkloadIdentity;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(super) struct Packs {
    /// Increases on every change to what plugins may do. A run that started
    /// under an older value is cancelled and its result is discarded.
    epoch: Arc<AtomicU64>,
    /// Latest observation per server, plugin and contribution.
    observations: Mutex<HashMap<(ServerId, String, String), PackObservation>>,
}

impl Workspace {
    /// Called after every change to plugin permissions: stops pack commands
    /// in flight and forgets what packs observed.
    pub(super) fn plugins_changed(&self) {
        self.plugins_changed_for(None);
    }

    /// What plugins may do changed, for one plugin or (`None`) for all. Pack
    /// commands, tunnels and views stop for every plugin; job runs stop only
    /// for the plugin concerned, so one plugin's change never ends another's
    /// unattended run.
    pub(super) fn plugins_changed_for(&self, plugin: Option<&str>) {
        self.packs.epoch.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut observations) = self.packs.observations.lock() {
            observations.clear();
        }
        self.close_pack_tunnels();
        // Plugin views close too, and their waiting requests are cancelled.
        self.close_plugin_views();
        // Job runs stop too; their waiting requests end.
        self.cancel_jobs(plugin);
    }

    /// One observation with its freshness as of now.
    fn observation(
        observations: &HashMap<(ServerId, String, String), PackObservation>,
        key: &(ServerId, String, String),
    ) -> Option<PackObservation> {
        let now = chrono::Utc::now();
        observations.get(key).cloned().map(|mut observation| {
            observation.fresh = atlas_core::observation_time(&observation.observed_at, now)
                .is_ok_and(|at| {
                    now.signed_duration_since(at).num_seconds() <= atlas_core::STALE_AFTER_SECONDS
                });
            observation
        })
    }

    /// What packs add to the map: a badge per allowed server, from the
    /// latest observation in memory, and the relationship kinds on offer.
    /// It reads saved settings and memory only and contacts no server.
    pub fn pack_map(&self) -> Result<PackMapState, AtlasError> {
        let directory = self.plugins_directory();
        let (bindings, relationship_kinds) = self.with_store(|store| {
            Ok((
                store.pack_badges(&directory)?,
                store.relationship_kinds(&directory)?,
            ))
        })?;
        let observations = self.packs.observations.lock().map_err(|_| failure())?;
        let badges = bindings
            .into_iter()
            .map(|binding| {
                let key = (
                    binding.server_id.clone(),
                    binding.plugin_id.clone(),
                    binding.observation,
                );
                let observation = Self::observation(&observations, &key);
                let (state, value) = atlas_core::pack::badge_state(observation.as_ref());
                PackBadgeView {
                    server_id: binding.server_id,
                    plugin_id: binding.plugin_id,
                    plugin_name: binding.plugin_name,
                    contribution_id: binding.contribution_id,
                    title: binding.title,
                    state,
                    value,
                    observed_at: observation.map(|observation| observation.observed_at),
                }
            })
            .collect();
        Ok(PackMapState {
            contract_version: CONTRACT_VERSION,
            badges,
            relationship_kinds,
        })
    }

    /// Documents a relationship, with a plugin kind if one was chosen.
    pub fn add_relationship(&self, input: AddRelationship) -> Result<MapState, AtlasError> {
        let directory = self.plugins_directory();
        self.with_store(|store| {
            store.add_relationship_with_kinds(&directory, input)?;
            store.snapshot()
        })
    }

    /// The pack applications the owner allowed on this server.
    pub fn pack_apps(&self, id: &ServerId) -> Result<PackAppsState, AtlasError> {
        let directory = self.plugins_directory();
        Ok(PackAppsState {
            contract_version: CONTRACT_VERSION,
            server_id: id.clone(),
            apps: self.with_store(|store| store.pack_apps(&directory, id))?,
        })
    }

    /// Open a pack application. The port and path come from the manifest
    /// the owner reviewed, the destination is always the server's own
    /// localhost, and the tunnel is the same loopback-only forward, with
    /// the same checks, as any other application.
    pub fn open_pack_app(
        self: &Arc<Self>,
        id: &ServerId,
        plugin: &str,
        contribution: &str,
    ) -> Result<TunnelInfo, AtlasError> {
        let directory = self.plugins_directory();
        let epoch = self.packs.epoch.load(Ordering::SeqCst);
        let app = self
            .with_store(|store| store.plugin_begin_app(&directory, plugin, id, contribution))?
            .map_err(|_| AtlasError::new(ErrorCode::Denied))?;
        let opened = self.open_tunnel(
            id,
            TunnelRequest {
                remote_port: app.port,
                path: app.path.clone(),
                remote_host: None,
            },
        );
        let opened = opened.and_then(|info| {
            self.mark_pack_tunnel(&info.tunnel_id)?;
            // The permissions may have changed while the tunnel started.
            if self.packs.epoch.load(Ordering::SeqCst) != epoch {
                self.close_pack_tunnels();
                return Err(AtlasError::new(ErrorCode::Cancelled));
            }
            Ok(info)
        });
        let logged = match &opened {
            Ok(_) => ActivityOutcome::Done,
            Err(failure) if failure.code == ErrorCode::Cancelled => ActivityOutcome::Cancelled,
            Err(_) => ActivityOutcome::Failed,
        };
        self.with_store(|store| store.plugin_finish_app(&app, id, logged))?;
        opened
    }

    /// The pack commands the owner allowed on this server, each with its
    /// latest observation from this run of InfraWerf.
    pub fn pack_state(&self, id: &ServerId) -> Result<PackServerState, AtlasError> {
        let directory = self.plugins_directory();
        let (runs, workloads) = self.with_store(|store| {
            let runs = store.pack_commands(&directory, id)?;
            let snapshot = store.snapshot()?;
            let server = snapshot
                .servers
                .iter()
                .find(|server| server.id == *id)
                .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
            Ok((
                runs,
                WorkloadIdentity::from_inventory(id, server.inventory.as_ref()),
            ))
        })?;
        let observations = self.packs.observations.lock().map_err(|_| failure())?;
        let commands = runs
            .into_iter()
            .map(|run| {
                let key = (
                    id.clone(),
                    run.plugin_id.clone(),
                    run.contribution.id.clone(),
                );
                let observation = Self::observation(&observations, &key);
                PackCommandView {
                    plugin_id: run.plugin_id,
                    plugin_name: run.plugin_name,
                    contribution_id: run.contribution.id,
                    title: run.contribution.title,
                    health_check: run.contribution.point
                        == atlas_core::extensions::ExtensionPoint::HealthCheck,
                    argv: run.command.argv.clone(),
                    parameters: run
                        .command
                        .parameters
                        .iter()
                        .map(|parameter| PackParameterView {
                            name: parameter.name.clone(),
                            source: parameter.source,
                            choices: atlas_core::pack::choices(parameter.source, &workloads),
                        })
                        .collect(),
                    workload_kind: run.contribution.workload_kind.clone(),
                    observation,
                }
            })
            .collect();
        Ok(PackServerState::new(id.clone(), commands))
    }

    /// Run one pack command now. Refused unless the owner's settings admit
    /// every capability it uses on this server at this moment; cancelled if
    /// those settings change before it finishes.
    pub fn run_pack_command(
        self: &Arc<Self>,
        id: &ServerId,
        plugin: &str,
        contribution: &str,
        parameters: &BTreeMap<String, String>,
    ) -> Result<PackServerState, AtlasError> {
        let _contact = self.live_contact_guard()?;
        let _admission = self.begin()?;
        let directory = self.plugins_directory();
        // Read before the check, so a change that lands after it is seen.
        let epoch = self.packs.epoch.load(Ordering::SeqCst);
        let run = self
            .with_store(|store| store.plugin_begin_run(&directory, plugin, id, contribution))?
            .map_err(|_| AtlasError::new(ErrorCode::Denied))?;
        let outcome = self.run_admitted(id, &run, parameters, epoch);
        let changed = self.packs.epoch.load(Ordering::SeqCst) != epoch;
        let logged = match &outcome {
            _ if changed => ActivityOutcome::Cancelled,
            Ok(observation) if observation.result.outcome == PackOutcome::Ok => {
                ActivityOutcome::Done
            }
            Err(failure) if failure.code == ErrorCode::Cancelled => ActivityOutcome::Cancelled,
            _ => ActivityOutcome::Failed,
        };
        self.with_store(|store| store.plugin_finish_run(&run, id, logged))?;
        if changed {
            return Err(AtlasError::new(ErrorCode::Cancelled));
        }
        let observation = outcome?;
        self.packs
            .observations
            .lock()
            .map_err(|_| failure())?
            .insert(
                (id.clone(), run.plugin_id, run.contribution.id),
                observation,
            );
        // The permissions may have changed while the result was stored.
        if self.packs.epoch.load(Ordering::SeqCst) != epoch {
            self.plugins_changed();
            return Err(AtlasError::new(ErrorCode::Cancelled));
        }
        self.pack_state(id)
    }

    fn run_admitted(
        self: &Arc<Self>,
        id: &ServerId,
        run: &PackRun,
        parameters: &BTreeMap<String, String>,
        epoch: u64,
    ) -> Result<PackObservation, AtlasError> {
        let target = self.ssh_target(id)?;
        let ticket = self.files.prepare(&target)?;
        let session_admission = self.begin()?;
        // The saved inventory lets the session rebuild the workloads it may
        // name, and only if this same authenticated connection collected it.
        let inventory = self.with_store(|store| {
            Ok(store
                .snapshot()?
                .servers
                .into_iter()
                .find(|server| server.id == *id)
                .and_then(|server| server.inventory))
        })?;
        let session = self.files.open(
            &target,
            &ticket.operation_id,
            Box::new(session_admission),
            inventory.as_ref(),
        )?;
        let current = self.packs.epoch.clone();
        let result: Result<(PackResult, BTreeMap<String, String>), AtlasError> =
            atlas_core::pack::resolve(&run.command, parameters, &session.workloads).and_then(
                |(argv, names)| {
                    self.files
                        .run_pack_command(
                            &target,
                            &session.session_id,
                            run.command.clone(),
                            argv,
                            Arc::new(move || current.load(Ordering::SeqCst) == epoch),
                        )
                        .map(|result| (result, names))
                },
            );
        let closed = self.files.close(&session.session_id);
        let (result, names) = result?;
        closed?;
        if self.ssh_target(id)? != target {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        Ok(PackObservation {
            observed_at: chrono::Utc::now().to_rfc3339(),
            fresh: true,
            source: format!("{}.{}", run.plugin_id, run.contribution.id),
            source_binding: atlas_core::inventory::authenticated_source_binding(
                &session.profile,
                &session.fingerprint,
            ),
            parameters: names,
            result,
        })
    }
}
