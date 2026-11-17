//! What the interface sees of plugin jobs (S12/F24/T049): runs, the owner's
//! confirmations and each server's jobs. The engine is in the desktop shell
//! (`workspace/jobs.rs`); the rules are in `plugin_host` and D061.
use crate::plugin::{JobControl, JobTarget};
use crate::plugin_host::JobEntry;
use crate::plugin_sandbox::SandboxOutcome;
use crate::{CONTRACT_VERSION, ServerId};
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

/// How long a run waits for the owner's answer before the action is refused.
pub const ASK_TIMEOUT_SECONDS: u64 = 5 * 60;
/// Longest a run may take, waiting for the owner included.
pub const RUN_DEADLINE_SECONDS: u64 = 10 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum JobRunState {
    Running,
    Done,
    Failed,
    Cancelled,
}

/// What became of one action a run asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct JobActionRecord {
    pub action_id: String,
    pub workload: String,
    /// `done`, `failed`, `declined`, `refused` or `unknown`.
    pub outcome: String,
    pub message: String,
}

/// One run of a job, for the Plugin data tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct JobRunView {
    pub run_id: String,
    pub plugin_id: String,
    pub contribution_id: String,
    pub server_id: ServerId,
    /// Started by the owner (true) or by a schedule.
    pub attended: bool,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub state: JobRunState,
    pub outcome: Option<SandboxOutcome>,
    pub actions: Vec<JobActionRecord>,
}

/// What InfraWerf shows the owner before a job's action runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct JobAsk {
    pub id: String,
    pub run_id: String,
    pub plugin_name: String,
    pub server_name: String,
    pub job_title: String,
    pub action_title: String,
    pub effect: String,
    pub control: JobControl,
    pub target: JobTarget,
    pub workload: String,
    /// The workload needs sudo: the owner types the password, which is used
    /// for this action only and never kept.
    pub requires_sudo: bool,
}

/// A server's jobs, with each one's latest run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct JobsState {
    pub contract_version: u32,
    pub server_id: ServerId,
    pub jobs: Vec<JobEntry>,
    pub runs: Vec<JobRunView>,
}

impl JobsState {
    pub fn new(server_id: ServerId, jobs: Vec<JobEntry>, runs: Vec<JobRunView>) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            server_id,
            jobs,
            runs,
        }
    }
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        JobRunState::decl(config),
        JobActionRecord::decl(config),
        JobRunView::decl(config),
        JobAsk::decl(config),
        JobsState::decl(config),
    ]
    .into_iter()
    .map(|d| format!("export {d}\n"))
    .collect()
}
