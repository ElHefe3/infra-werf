//! Plugin jobs owned by the workspace (F24/T049, owner decision D061).
//!
//! A job is a plugin's WebAssembly module run in the sandbox (T048) on one
//! server, started by the owner or by a schedule the owner set. Its host
//! functions are decided here against the owner's grants at the moment of
//! each call. A changing action is one of InfraWerf's own workload controls,
//! declared in the manifest. Each action:
//!
//! - runs at once only at trusted automation, and only if it needs no sudo;
//! - otherwise shows InfraWerf's own dialog, in a run the owner started;
//! - is refused in a scheduled run.
//!
//! Sudo needs the owner's named exception and the password, every time.
//! Schedules belong to the core: they fire only while InfraWerf is open and
//! plugins may act, and pause after repeated failures.
use super::*;
use atlas_core::extensions::Capability;
use atlas_core::plugin::{JobControl, JobTarget};
use atlas_core::plugin_host::{
    ACTION_JOB_ACTION, ACTION_JOB_RUN, ActivityOutcome, DueSchedule, JobSource, Refusal,
};
use atlas_core::plugin_jobs::{
    ASK_TIMEOUT_SECONDS, JobActionRecord, JobAsk, JobRunState, JobRunView, JobsState,
    RUN_DEADLINE_SECONDS,
};
use atlas_core::plugin_sandbox::{HostError, SandboxHost, SandboxLimits, SandboxStop};
use atlas_core::workloads::{WorkloadAction, WorkloadKind, workload_control_requires_sudo};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The owner's answer to a job's request.
pub struct JobAnswer {
    pub run: bool,
    pub password: Option<ActionPassword>,
}

#[derive(Default)]
pub(super) struct Jobs {
    runs: Mutex<HashMap<String, JobRunView>>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    asks: Mutex<HashMap<String, (JobAsk, mpsc::Sender<JobAnswer>)>>,
    last_tick: Mutex<Option<Instant>>,
}

type AskEmitter = Box<dyn Fn(JobAsk) + Send + Sync>;
static ASK_EMITTER: std::sync::OnceLock<AskEmitter> = std::sync::OnceLock::new();

fn error(code: &str, message: &str) -> HostError {
    HostError::new(code, message)
}

fn refusal_word(refusal: Refusal) -> String {
    serde_json::to_value(refusal)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// One run's link to InfraWerf.
struct JobHost {
    workspace: Arc<Workspace>,
    source: JobSource,
    server: ServerId,
    run_id: String,
    attended: bool,
    cancel: Arc<AtomicBool>,
    deadline: Instant,
    actions: Vec<JobActionRecord>,
}

impl JobHost {
    fn plugin(&self) -> &str {
        &self.source.entry.plugin_id
    }

    fn log_action(&self, outcome: ActivityOutcome, action: &str, workload: &str, word: &str) {
        let directory = self.workspace.plugins_directory();
        let _ = self.workspace.with_store(|store| {
            store.plugin_job_event(
                &directory,
                self.plugin(),
                &self.server,
                ACTION_JOB_ACTION,
                outcome,
                &format!("{action}: {workload}: {word}"),
            )
        });
    }

    fn record(&mut self, action: &str, workload: &str, outcome: &str, message: &str) {
        self.actions.push(JobActionRecord {
            action_id: action.into(),
            workload: workload.into(),
            outcome: outcome.into(),
            message: message.into(),
        });
    }

    fn read_allowed(&self) -> bool {
        self.workspace
            .plugin_authorize(self.plugin(), &self.server, Capability::ReadSavedState)
            .is_ok_and(|d| d.allowed && !d.needs_confirmation)
    }

    fn saved_server(&self) -> Result<MapServer, HostError> {
        self.workspace
            .with_store(|store| store.snapshot())
            .ok()
            .and_then(|state| state.servers.into_iter().find(|s| s.id == self.server))
            .ok_or_else(|| error("unknown_server", "The server is no longer on the map."))
    }

    /// Waits for the owner's answer in InfraWerf's own dialog.
    fn ask_owner(&self, ask: JobAsk) -> Option<JobAnswer> {
        let (tx, rx) = mpsc::channel();
        let id = ask.id.clone();
        self.workspace
            .jobs
            .asks
            .lock()
            .ok()?
            .insert(id.clone(), (ask.clone(), tx));
        if let Some(emit) = ASK_EMITTER.get() {
            emit(ask);
        }
        let until = Instant::now() + Duration::from_secs(ASK_TIMEOUT_SECONDS);
        let answer = loop {
            if self.cancel.load(Ordering::SeqCst) || Instant::now() >= until.min(self.deadline) {
                break None;
            }
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(answer) => break Some(answer),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };
        if let Ok(mut asks) = self.workspace.jobs.asks.lock() {
            asks.remove(&id);
        }
        answer
    }

    fn control(&mut self, args: &Value) -> Result<Value, HostError> {
        let action_id = args
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        // A workload by its id, or by its name among the server's workloads
        // of the action's kind (a module need not parse the inventory).
        let workload_id = args
            .get("workload_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let workload_name = args
            .get("workload")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let Some(action) = self
            .source
            .entry
            .job
            .actions
            .iter()
            .find(|a| a.id == action_id)
            .cloned()
        else {
            return Err(error("unknown_action", "This job declares no such action."));
        };
        let directory = self.workspace.plugins_directory();
        let decision = self
            .workspace
            .with_store(|store| store.plugin_decide_change(&directory, self.plugin(), &self.server))
            .map_err(|_| error("unavailable", "InfraWerf could not decide this action."))?;
        let needs_confirmation = match decision {
            Ok(confirm) => confirm,
            Err(refusal) => {
                let word = refusal_word(refusal);
                self.log_action(ActivityOutcome::Refused, &action.id, "", &word);
                self.record(&action.id, "", "refused", &word);
                return Err(error(
                    &word,
                    "The owner has not allowed this plugin to control workloads on this server.",
                ));
            }
        };
        // The workload must be one this server's authenticated discovery
        // found, of the action's kind.
        let target = self
            .workspace
            .ssh_target(&self.server)
            .map_err(|_| error("unavailable", "The server has no SSH connection set up."))?;
        let ticket = self
            .workspace
            .files
            .prepare(&target)
            .map_err(|_| error("unavailable", "InfraWerf could not connect."))?;
        let admission = self
            .workspace
            .begin()
            .map_err(|_| error("unavailable", "InfraWerf is busy or closing."))?;
        let inventory = self.saved_server().ok().and_then(|s| s.inventory);
        let session = self
            .workspace
            .files
            .open(
                &target,
                &ticket.operation_id,
                Box::new(admission),
                inventory.as_ref(),
            )
            .map_err(|_| {
                error(
                    "unavailable",
                    "InfraWerf could not open an authenticated session.",
                )
            })?;
        let result = self.control_in(
            &session,
            &action,
            &workload_id,
            &workload_name,
            needs_confirmation,
        );
        let _ = self.workspace.files.close(&session.session_id);
        result
    }

    fn control_in(
        &mut self,
        session: &atlas_core::files::FileSession,
        action: &atlas_core::plugin::JobAction,
        workload_id: &str,
        workload_name: &str,
        needs_confirmation: bool,
    ) -> Result<Value, HostError> {
        let of_kind = |w: &&atlas_core::workloads::WorkloadIdentity| {
            matches!(
                (action.target, w.kind),
                (JobTarget::Service, WorkloadKind::Service)
                    | (JobTarget::Container, WorkloadKind::Container)
            )
        };
        let found: Vec<_> = if !workload_id.is_empty() {
            session
                .workloads
                .iter()
                .filter(|w| w.id == workload_id)
                .collect()
        } else if !workload_name.is_empty() {
            session
                .workloads
                .iter()
                .filter(of_kind)
                .filter(|w| w.name == workload_name)
                .collect()
        } else {
            Vec::new()
        };
        let asked = if workload_id.is_empty() {
            workload_name
        } else {
            workload_id
        };
        if found.len() > 1 {
            self.record(&action.id, asked, "refused", "ambiguous_workload");
            return Err(error(
                "ambiguous_workload",
                "More than one workload has that name; send its workload_id.",
            ));
        }
        let Some(workload) = found.first().map(|w| (*w).clone()) else {
            self.record(&action.id, asked, "refused", "unknown_workload");
            return Err(error(
                "unknown_workload",
                "Not a workload this server's authenticated discovery found.",
            ));
        };
        let kind_matches = matches!(
            (action.target, workload.kind),
            (JobTarget::Service, WorkloadKind::Service)
                | (JobTarget::Container, WorkloadKind::Container)
        );
        if !kind_matches || !workload.supported {
            self.record(&action.id, &workload.name, "refused", "wrong_target");
            return Err(error(
                "wrong_target",
                "This action does not act on that kind of workload.",
            ));
        }
        let requires_sudo = workload_control_requires_sudo(&workload);
        let refuse = |host: &mut Self, word: &str, message: &str| {
            host.log_action(ActivityOutcome::Refused, &action.id, &workload.name, word);
            host.record(&action.id, &workload.name, "refused", word);
            Err(error(word, message))
        };
        if requires_sudo {
            let directory = self.workspace.plugins_directory();
            let granted = self
                .workspace
                .with_store(|store| {
                    store.plugin_has_elevation(self.plugin(), &self.server, &action.id)
                })
                .unwrap_or(false);
            let _ = directory;
            if !granted {
                return refuse(
                    self,
                    "no_elevation_exception",
                    "This action needs sudo, and the owner has not granted this plugin an exception for it on this server.",
                );
            }
            if !self.attended {
                return refuse(
                    self,
                    "sudo_never_unattended",
                    "An action that needs sudo runs only when the owner starts the run and types the password.",
                );
            }
        } else if needs_confirmation && !self.attended {
            return refuse(
                self,
                "needs_confirmation",
                "Scheduled runs make changes only under trusted automation; this one needs the owner's confirmation.",
            );
        }
        let control = match action.control {
            JobControl::Start => WorkloadAction::Start,
            JobControl::Stop => WorkloadAction::Stop,
            JobControl::Restart => WorkloadAction::Restart,
        };
        let preview = self
            .workspace
            .preview_workload_control(&self.server, &session.session_id, &workload.id, control)
            .map_err(|_| error("unavailable", "InfraWerf could not prepare the action."))?;
        let mut password = None;
        if needs_confirmation || requires_sudo {
            let server_name = self.saved_server().map(|s| s.name).unwrap_or_default();
            let ask = JobAsk {
                id: OperationId::generate().as_str().to_owned(),
                run_id: self.run_id.clone(),
                plugin_name: self.source.entry.plugin_name.clone(),
                server_name,
                job_title: self.source.entry.title.clone(),
                action_title: action.title.clone(),
                effect: action.effect.clone(),
                control: action.control,
                target: action.target,
                workload: workload.name.clone(),
                requires_sudo,
            };
            self.log_action(
                ActivityOutcome::NeedsConfirmation,
                &action.id,
                &workload.name,
                "asked you",
            );
            match self.ask_owner(ask) {
                Some(answer) if answer.run => password = answer.password,
                Some(_) => return refuse(self, "declined", "The owner declined this action."),
                None if self.cancel.load(Ordering::SeqCst) => {
                    return refuse(self, "cancelled", "The run was cancelled.");
                }
                None => return refuse(self, "no_answer", "The owner did not answer in time."),
            }
            if requires_sudo && password.is_none() {
                return refuse(self, "no_password", "This action needs the sudo password.");
            }
        }
        if self.cancel.load(Ordering::SeqCst) {
            return refuse(self, "cancelled", "The run was cancelled.");
        }
        match self.workspace.confirm_workload_control(
            &self.server,
            &session.session_id,
            &preview.operation_id,
            password,
        ) {
            Ok(receipt) => {
                let outcome = serde_json::to_value(receipt.outcome)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let done = outcome == "completed";
                self.log_action(
                    if done {
                        ActivityOutcome::Done
                    } else {
                        ActivityOutcome::Failed
                    },
                    &action.id,
                    &workload.name,
                    &outcome,
                );
                self.record(
                    &action.id,
                    &workload.name,
                    if done { "done" } else { "failed" },
                    &receipt.message,
                );
                Ok(serde_json::json!({ "outcome": outcome, "state": receipt.observed_state }))
            }
            Err(failure) => {
                let word = serde_json::to_value(failure.code)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                self.log_action(ActivityOutcome::Failed, &action.id, &workload.name, &word);
                self.record(&action.id, &workload.name, "failed", &word);
                Err(error("failed", "The action could not be completed."))
            }
        }
    }
}

impl SandboxHost for JobHost {
    fn call(
        &mut self,
        function: &str,
        capability: Capability,
        args: &Value,
    ) -> Result<Value, HostError> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(error("cancelled", "The run was cancelled."));
        }
        if Instant::now() >= self.deadline {
            return Err(error("too_long", "The run has used all its time."));
        }
        match (function, capability) {
            ("server" | "inventory" | "workloads", Capability::ReadSavedState) => {
                if !self.read_allowed() {
                    return Err(error(
                        "not_granted",
                        "The owner has not allowed this plugin to do that on this server.",
                    ));
                }
                let server = self.saved_server()?;
                Ok(match function {
                    "server" => serde_json::json!({ "id": server.id, "name": server.name, "host": server.host }),
                    "inventory" => serde_json::to_value(&server.inventory).unwrap_or(Value::Null),
                    _ => Value::Array(
                        atlas_core::workloads::WorkloadIdentity::from_inventory(&self.server, server.inventory.as_ref())
                            .into_iter()
                            .map(|w| serde_json::json!({ "id": w.id, "name": w.name, "kind": w.kind, "scope": w.scope, "runtime": w.runtime, "state": w.reported_state }))
                            .collect(),
                    ),
                })
            }
            ("run_command", Capability::RunApprovedCommand) => {
                let contribution = args
                    .get("contribution_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let parameters: BTreeMap<String, String> = args
                    .get("parameters")
                    .and_then(|p| serde_json::from_value(p.clone()).ok())
                    .unwrap_or_default();
                match self.workspace.run_pack_command(
                    &self.server,
                    self.plugin(),
                    contribution,
                    &parameters,
                ) {
                    Ok(state) => Ok(state
                        .commands
                        .into_iter()
                        .find(|c| c.plugin_id == self.plugin() && c.contribution_id == contribution)
                        .and_then(|c| c.observation)
                        .map(|o| serde_json::to_value(o).unwrap_or(Value::Null))
                        .unwrap_or(Value::Null)),
                    Err(failure) if failure.code == ErrorCode::Cancelled => {
                        Err(error("cancelled", "The run was cancelled."))
                    }
                    Err(_) => Err(error(
                        "not_granted",
                        "That command is not allowed on this server, or a parameter is not a discovered workload.",
                    )),
                }
            }
            ("control", Capability::ControlWorkloads) => self.control(args),
            _ => Err(error("unknown_function", "There is no such host function.")),
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) || Instant::now() >= self.deadline
    }
}

impl Workspace {
    pub fn set_job_ask_emitter(&self, emit: AskEmitter) {
        let _ = ASK_EMITTER.set(emit);
    }

    /// A server's jobs, with their latest runs.
    pub fn plugin_jobs(&self, server: &ServerId) -> Result<JobsState, AtlasError> {
        let directory = self.plugins_directory();
        let jobs = self
            .with_store(|store| store.plugin_jobs(&directory, server))?
            .into_iter()
            .map(|source| source.entry)
            .collect();
        let mut runs: Vec<JobRunView> = self
            .jobs
            .runs
            .lock()
            .map_err(|_| failure())?
            .values()
            .filter(|run| &run.server_id == server)
            .cloned()
            .collect();
        runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(JobsState::new(server.clone(), jobs, runs))
    }

    /// Starts a run of a job now. `attended` is true when the owner pressed
    /// Run; a schedule passes false. Returns the run's ID.
    pub fn start_job(
        self: &Arc<Self>,
        server: &ServerId,
        plugin: &str,
        contribution: &str,
        attended: bool,
        schedule: Option<DueSchedule>,
    ) -> Result<String, AtlasError> {
        let directory = self.plugins_directory();
        let source = self
            .with_store(|store| store.plugin_jobs(&directory, server))?
            .into_iter()
            .find(|s| s.entry.plugin_id == plugin && s.entry.contribution_id == contribution)
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        // One run of a job per server at a time.
        let busy = self
            .jobs
            .runs
            .lock()
            .map_err(|_| failure())?
            .values()
            .any(|r| {
                r.state == JobRunState::Running
                    && r.plugin_id == plugin
                    && r.contribution_id == contribution
                    && &r.server_id == server
            });
        if busy {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        // The module that runs is the one installed and checked.
        let files = atlas_core::plugin_install::read_folder(&source.folder)
            .map_err(|_| AtlasError::new(ErrorCode::Denied))?;
        if source
            .content_sha256
            .as_deref()
            .is_some_and(|recorded| recorded != atlas_core::plugin_install::content_hash(&files))
        {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let module = files
            .into_iter()
            .find(|(path, _)| *path == source.entry.job.module)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        let run_id = OperationId::generate().as_str().to_owned();
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs
            .cancels
            .lock()
            .map_err(|_| failure())?
            .insert(run_id.clone(), cancel.clone());
        let view = JobRunView {
            run_id: run_id.clone(),
            plugin_id: plugin.into(),
            contribution_id: contribution.into(),
            server_id: server.clone(),
            attended,
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            state: JobRunState::Running,
            outcome: None,
            actions: Vec::new(),
        };
        self.jobs
            .runs
            .lock()
            .map_err(|_| failure())?
            .insert(run_id.clone(), view);
        let workspace = self.clone();
        let (server, id) = (server.clone(), run_id.clone());
        std::thread::Builder::new()
            .name("plugin-job".into())
            .spawn(move || {
                workspace.run_job(source, server, id, attended, cancel, module, schedule)
            })
            .map_err(|_| failure())?;
        Ok(run_id)
    }

    fn run_job(
        self: Arc<Self>,
        source: JobSource,
        server: ServerId,
        run_id: String,
        attended: bool,
        cancel: Arc<AtomicBool>,
        module: Vec<u8>,
        schedule: Option<DueSchedule>,
    ) {
        let plugin = source.entry.plugin_id.clone();
        let contribution = source.entry.contribution_id.clone();
        let mut host = JobHost {
            workspace: self.clone(),
            source,
            server: server.clone(),
            run_id: run_id.clone(),
            attended,
            cancel: cancel.clone(),
            deadline: Instant::now() + Duration::from_secs(RUN_DEADLINE_SECONDS),
            actions: Vec::new(),
        };
        let outcome = atlas_core::plugin_sandbox::run(&module, &mut host, SandboxLimits::default());
        let state = match outcome.stopped {
            None if outcome.exit_code == Some(0) => JobRunState::Done,
            Some(SandboxStop::Cancelled) => JobRunState::Cancelled,
            _ if cancel.load(Ordering::SeqCst) => JobRunState::Cancelled,
            _ => JobRunState::Failed,
        };
        let word = match state {
            JobRunState::Done => "done".to_owned(),
            JobRunState::Cancelled => "cancelled".to_owned(),
            _ => match (outcome.stopped, outcome.exit_code) {
                (Some(stop), _) => format!(
                    "failed: {}",
                    serde_json::to_value(stop)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default()
                ),
                (None, Some(code)) => format!("failed: exit {code}"),
                (None, None) => "failed".to_owned(),
            },
        };
        let directory = self.plugins_directory();
        let _ = self.with_store(|store| {
            store.plugin_job_event(
                &directory,
                &plugin,
                &server,
                ACTION_JOB_RUN,
                match state {
                    JobRunState::Done => ActivityOutcome::Done,
                    JobRunState::Cancelled => ActivityOutcome::Cancelled,
                    _ => ActivityOutcome::Failed,
                },
                &format!(
                    "{contribution}: {}{word}",
                    if attended { "" } else { "scheduled, " }
                ),
            )
        });
        if let Some(due) = &schedule {
            let outcome_word = match state {
                JobRunState::Done => "done",
                JobRunState::Cancelled => "cancelled",
                _ => "failed",
            };
            let _ = self
                .with_store(|store| store.plugin_schedule_finished(&directory, due, outcome_word));
        }
        if let Ok(mut runs) = self.jobs.runs.lock() {
            if let Some(view) = runs.get_mut(&run_id) {
                view.state = state;
                view.finished_at = Some(chrono::Utc::now().to_rfc3339());
                view.outcome = Some(outcome);
                view.actions = std::mem::take(&mut host.actions);
            }
            // Keep the latest runs only.
            if runs.len() > 50 {
                let mut finished: Vec<(String, String)> = runs
                    .values()
                    .filter(|r| r.state != JobRunState::Running)
                    .map(|r| (r.started_at.clone(), r.run_id.clone()))
                    .collect();
                finished.sort();
                for (_, id) in finished.into_iter().take(runs.len() - 50) {
                    runs.remove(&id);
                }
            }
        }
        if let Ok(mut cancels) = self.jobs.cancels.lock() {
            cancels.remove(&run_id);
        }
    }

    /// The owner's answer to a job's request.
    pub fn answer_job(&self, id: &str, answer: JobAnswer) -> Result<(), AtlasError> {
        let entry = self.jobs.asks.lock().map_err(|_| failure())?.remove(id);
        let (_, sender) = entry.ok_or_else(|| AtlasError::new(ErrorCode::NotFound))?;
        sender
            .send(answer)
            .map_err(|_| AtlasError::new(ErrorCode::NotFound))
    }

    /// Requests waiting for the owner, for an interface that missed the event.
    pub fn job_asks(&self) -> Vec<JobAsk> {
        self.jobs
            .asks
            .lock()
            .map(|asks| asks.values().map(|(ask, _)| ask.clone()).collect())
            .unwrap_or_default()
    }

    /// Stops a run: its next host call fails and its waiting request ends.
    pub fn cancel_job(&self, run_id: &str) {
        if let Some(flag) = self
            .jobs
            .cancels
            .lock()
            .ok()
            .and_then(|c| c.get(run_id).cloned())
        {
            flag.store(true, Ordering::SeqCst);
        }
    }

    /// Called whenever what plugins may do changes: the runs of that plugin
    /// (or of every plugin, for `None`) stop.
    pub(super) fn cancel_jobs(&self, plugin: Option<&str>) {
        let concerned: Option<Vec<String>> = plugin.map(|plugin| {
            self.jobs
                .runs
                .lock()
                .map(|runs| {
                    runs.values()
                        .filter(|run| run.plugin_id == plugin)
                        .map(|run| run.run_id.clone())
                        .collect()
                })
                .unwrap_or_default()
        });
        if let Ok(cancels) = self.jobs.cancels.lock() {
            for (run_id, flag) in cancels.iter() {
                if concerned.as_ref().is_none_or(|ids| ids.contains(run_id)) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }
    }

    /// The scheduler: fires due schedules, at most every two seconds. A
    /// schedule's next time is set when it starts, so nothing runs twice.
    pub fn tick_jobs(self: &Arc<Self>) {
        {
            let Ok(mut last) = self.jobs.last_tick.lock() else {
                return;
            };
            if last.is_some_and(|at| at.elapsed() < Duration::from_secs(2)) {
                return;
            }
            *last = Some(Instant::now());
        }
        let now = chrono::Utc::now();
        let Ok(due) = self.with_store(|store| store.plugin_due_schedules(now)) else {
            return;
        };
        let directory = self.plugins_directory();
        for schedule in due {
            let _ = self.with_store(|store| store.plugin_schedule_started(&schedule, now));
            let started = self.start_job(
                &schedule.server_id,
                &schedule.plugin_id,
                &schedule.contribution_id,
                false,
                Some(schedule.clone()),
            );
            if started.is_err() {
                let _ = self.with_store(|store| {
                    store.plugin_job_event(
                        &directory,
                        &schedule.plugin_id,
                        &schedule.server_id,
                        ACTION_JOB_RUN,
                        ActivityOutcome::Refused,
                        &format!("{}: scheduled, not allowed now", schedule.contribution_id),
                    )?;
                    store
                        .plugin_schedule_finished(&directory, &schedule, "refused")
                        .map(|_| ())
                });
            }
        }
    }
}
