//! Plugin jobs (T049, owner decision D061): the job manifest, the example
//! and fixture modules, schedules, sudo exceptions and the action decision.
use atlas_core::extensions::Capability;
use atlas_core::plugin::*;
use atlas_core::plugin_host::*;
use atlas_core::plugin_sandbox::*;
use atlas_core::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn repository(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

const MODULES: [&str; 2] = ["examples/service-summary", "scripts/fixtures/job-plugin"];

/// The committed `job.wasm` files are built from their `job.wat` sources.
/// `ATLAS_WRITE_WASM=1` rebuilds them.
#[test]
fn job_modules_match_their_sources() {
    for folder in MODULES {
        let built = wat::parse_file(repository(folder).join("job.wat"))
            .unwrap_or_else(|error| panic!("{folder}: {error}"));
        let committed = repository(folder).join("job.wasm");
        if std::env::var_os("ATLAS_WRITE_WASM").is_some() {
            std::fs::write(&committed, &built).unwrap();
        }
        assert_eq!(
            std::fs::read(&committed).unwrap_or_default(),
            built,
            "{folder}/job.wasm is stale: run with ATLAS_WRITE_WASM=1"
        );
    }
}

#[test]
fn the_example_and_fixture_manifests_are_valid_jobs() {
    for folder in MODULES {
        let manifest = validate_manifest(
            &std::fs::read(repository(folder).join("infrawerf-plugin.json")).unwrap(),
        )
        .unwrap_or_else(|issues| panic!("{folder}: {issues:?}"));
        let job = manifest.contributions[0].job.as_ref().unwrap();
        assert_eq!(job.module, "job.wasm");
    }
}

fn codes(manifest: &Value) -> Vec<String> {
    match validate_manifest(manifest.to_string().as_bytes()) {
        Ok(_) => Vec::new(),
        Err(issues) => issues.into_iter().map(|i| i.code.code()).collect(),
    }
}

/// The fixture manifest without one capability, everywhere it is listed.
fn keeper_without(capability: &str) -> Value {
    let mut m = keeper();
    m["capabilities"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c != capability);
    m["contributions"][0]["capabilities"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c != capability);
    m
}

fn keeper() -> Value {
    serde_json::from_slice(
        &std::fs::read(repository(
            "scripts/fixtures/job-plugin/infrawerf-plugin.json",
        ))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn a_job_manifest_is_refused_unless_every_rule_holds() {
    let job = |edit: &dyn Fn(&mut Value)| {
        let mut m = keeper();
        edit(&mut m["contributions"][0]);
        codes(&m)
    };
    assert!(job(&|_| {}).is_empty());
    // The job point needs a job, and only the job point may have one.
    assert_eq!(
        job(&|c| {
            c.as_object_mut().unwrap().remove("job");
        }),
        ["invalid_job"]
    );
    assert!(job(&|c| c["point"] = "workspace_tab".into()).contains(&"invalid_job".to_owned()));
    // The module is a clean relative path to a .wasm file.
    for module in ["../job.wasm", "/job.wasm", "job.wat", "a\\job.wasm", ""] {
        assert_eq!(
            job(&|c| c["job"]["module"] = module.into()),
            ["invalid_job"],
            "{module}"
        );
    }
    // Actions: tidy, unique, few, and only with permission to control.
    assert_eq!(
        job(&|c| c["job"]["actions"][1]["id"] = "restart-worker".into()),
        ["invalid_job"]
    );
    assert_eq!(
        job(&|c| c["job"]["actions"][0]["id"] = "Restart".into()),
        ["invalid_job"]
    );
    assert_eq!(
        job(&|c| c["job"]["actions"][0]["title"] = "x".repeat(41).into()),
        ["invalid_job"]
    );
    assert_eq!(
        job(&|c| c["job"]["actions"][0]["effect"] = "".into()),
        ["invalid_job"]
    );
    assert_eq!(
        job(&|c| {
            let action = c["job"]["actions"][0].clone();
            c["job"]["actions"] = (0..17)
                .map(|n| {
                    let mut a = action.clone();
                    a["id"] = format!("action-{n}").into();
                    a
                })
                .collect::<Vec<_>>()
                .into();
        }),
        ["invalid_job"]
    );
    assert_eq!(
        job(&|c| c["job"]["actions"][0]["control"] = "delete".into()),
        ["invalid_job"]
    );
    assert!(codes(&keeper_without("control_workloads")).contains(&"invalid_job".to_owned()));
    // A job is a v2 point.
    let mut v1 = keeper();
    v1["api_version"] = 1.into();
    assert!(!codes(&v1).is_empty());
}

/// A host that answers reads and refuses or accepts controls.
struct Host {
    control: Result<Value, (&'static str, &'static str)>,
    asked: Vec<Value>,
}

impl SandboxHost for Host {
    fn call(&mut self, function: &str, _: Capability, args: &Value) -> Result<Value, HostError> {
        self.asked
            .push(serde_json::json!({ "function": function, "args": args }));
        match function {
            "workloads" => {
                Ok(serde_json::json!({ "workloads": [{ "name": "fixture-worker.service" }] }))
            }
            "control" => self.control.clone().map_err(|(c, m)| HostError::new(c, m)),
            _ => Err(HostError::new("not_granted", "not allowed")),
        }
    }
    fn cancelled(&self) -> bool {
        false
    }
}

fn limits() -> SandboxLimits {
    SandboxLimits {
        fuel: 2_000_000,
        ..SandboxLimits::default()
    }
}

#[test]
fn the_example_job_reads_workloads_and_returns_them() {
    let mut host = Host {
        control: Err(("never", "")),
        asked: Vec::new(),
    };
    let module = std::fs::read(repository("examples/service-summary/job.wasm")).unwrap();
    let outcome = run(&module, &mut host, limits());
    assert_eq!(outcome.exit_code, Some(0), "{outcome:?}");
    let result = outcome.result.clone().unwrap();
    assert_eq!(
        result["ok"]["workloads"][0]["name"], "fixture-worker.service",
        "{result}"
    );
    assert_eq!(
        outcome.logs,
        ["service summary: reading this server's workloads"]
    );
    assert_eq!(host.asked.len(), 1);
}

#[test]
fn the_fixture_job_asks_for_both_actions_by_name_and_carries_on_when_refused() {
    let mut host = Host {
        control: Err((
            "needs_confirmation",
            "Scheduled runs make changes only under trusted automation.",
        )),
        asked: Vec::new(),
    };
    let module = std::fs::read(repository("scripts/fixtures/job-plugin/job.wasm")).unwrap();
    let outcome = run(&module, &mut host, limits());
    assert_eq!(outcome.exit_code, Some(0), "{outcome:?}");
    let controls: Vec<_> = host
        .asked
        .iter()
        .filter(|a| a["function"] == "control")
        .map(|a| a["args"].clone())
        .collect();
    assert_eq!(
        controls,
        [
            serde_json::json!({ "action": "restart-worker", "workload": "fixture-worker.service" }),
            serde_json::json!({ "action": "start-stopped", "workload": "fixture-stopped.service" }),
        ]
    );
    assert_eq!(outcome.result.unwrap()["error"], "needs_confirmation");
    assert!(
        outcome.logs[1].contains("needs_confirmation"),
        "{:?}",
        outcome.logs
    );
    assert_eq!(outcome.logs.last().unwrap(), "service keeper: done");
}

struct Fixture {
    _directory: tempfile::TempDir,
    plugins: PathBuf,
    store: Store,
    alpha: ServerId,
    beta: ServerId,
}

const KEEPER: &str = "org.example.service-keeper";
const JOB: &str = "keeper";

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
    }
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let plugins = directory.path().join("plugins");
    let mut store = Store::create(&directory.path().join("atlas.sqlite3")).unwrap();
    let mut add = |name: &str| {
        store
            .add_server(AddServer {
                name: name.into(),
                host: format!("{name}.invalid"),
            })
            .unwrap()
    };
    let (alpha, beta) = (add("alpha"), add("beta"));
    copy(
        &repository("scripts/fixtures/job-plugin"),
        &plugins.join("job-plugin"),
    );
    Fixture {
        _directory: directory,
        plugins,
        store,
        alpha,
        beta,
    }
}

impl Fixture {
    fn enable(&mut self, trust: TrustLevel, server: &ServerId, capabilities: &[Capability]) {
        self.store
            .plugin_enable(&self.plugins, KEEPER, trust)
            .unwrap();
        self.store
            .plugin_set_grant(&self.plugins, KEEPER, server, capabilities)
            .unwrap();
    }
    fn jobs(&self, server: &ServerId) -> Vec<JobEntry> {
        self.store
            .plugin_jobs(&self.plugins, server)
            .unwrap()
            .into_iter()
            .map(|s| s.entry)
            .collect()
    }
    fn schedule(&mut self, server: &ServerId, minutes: Option<u32>) -> Result<(), AtlasError> {
        let plugins = self.plugins.clone();
        self.store
            .plugin_set_schedule(&plugins, KEEPER, server, JOB, minutes)
    }
    fn decide(&self, server: &ServerId) -> Result<bool, Refusal> {
        self.store
            .plugin_decide_change(&self.plugins, KEEPER, server)
            .unwrap()
    }
    fn last(&self) -> PluginActivity {
        self.store.plugin_activity(5).unwrap().entries.remove(0)
    }
}

const ALL: [Capability; 3] = [
    Capability::ReadSavedState,
    Capability::SshSession,
    Capability::ControlWorkloads,
];

#[test]
fn a_job_is_offered_only_where_the_owner_allowed_its_reads() {
    let mut f = fixture();
    let (alpha, beta) = (f.alpha.clone(), f.beta.clone());
    assert!(f.jobs(&alpha).is_empty());
    f.enable(TrustLevel::ReadOnly, &alpha, &[Capability::ReadSavedState]);
    assert!(f.jobs(&alpha).is_empty(), "ssh_session is a read it lists");
    f.enable(
        TrustLevel::ReadOnly,
        &alpha,
        &[Capability::ReadSavedState, Capability::SshSession],
    );
    let jobs = f.jobs(&alpha);
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job.actions.len(), 2);
    assert_eq!(jobs[0].trust, TrustLevel::ReadOnly);
    assert!(f.jobs(&beta).is_empty());
    f.store.plugins_set_all_off(true).unwrap();
    assert!(f.jobs(&alpha).is_empty());
}

#[test]
fn an_action_is_refused_below_confirm_changes_and_asks_unless_trusted() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    assert_eq!(f.decide(&alpha), Err(Refusal::PluginDisabled));
    f.enable(TrustLevel::ReadOnly, &alpha, &ALL);
    assert_eq!(f.decide(&alpha), Err(Refusal::TrustTooLow));
    f.enable(TrustLevel::ConfirmChanges, &alpha, &ALL[..2]);
    assert_eq!(f.decide(&alpha), Err(Refusal::NotGranted));
    f.enable(TrustLevel::ConfirmChanges, &alpha, &ALL);
    assert_eq!(f.decide(&alpha), Ok(true), "the owner confirms");
    f.enable(TrustLevel::TrustedAutomation, &alpha, &ALL);
    assert_eq!(f.decide(&alpha), Ok(false), "runs without asking");
    assert_eq!(f.decide(&f.beta.clone()), Err(Refusal::NotGranted));
    f.store.plugins_set_all_off(true).unwrap();
    assert_eq!(f.decide(&alpha), Err(Refusal::AllPluginsOff));
}

#[test]
fn schedules_are_the_owners_per_server_and_pause_after_three_failures() {
    let mut f = fixture();
    let (alpha, beta) = (f.alpha.clone(), f.beta.clone());
    f.enable(TrustLevel::TrustedAutomation, &alpha, &ALL);
    for too_often in [0, 1, 4, 1441] {
        assert!(f.schedule(&alpha, Some(too_often)).is_err(), "{too_often}");
    }
    let plugins = f.plugins.clone();
    assert!(
        f.store
            .plugin_set_schedule(&plugins, KEEPER, &alpha, "nope", Some(5))
            .is_err()
    );
    f.schedule(&alpha, Some(5)).unwrap();
    assert_eq!(f.last().action, ACTION_SET_SCHEDULE);
    assert_eq!(f.last().detail, "keeper: every 5 minutes");
    let schedule = f.jobs(&alpha)[0].schedule.clone().unwrap();
    assert_eq!(schedule.every_minutes, 5);
    assert!(
        f.jobs(&beta).is_empty(),
        "beta has neither grants nor a schedule"
    );

    let now = chrono::Utc::now();
    assert!(f.store.plugin_due_schedules(now).unwrap().is_empty());
    let later = now + chrono::Duration::minutes(6);
    let due = f.store.plugin_due_schedules(later).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(
        (due[0].server_id.clone(), due[0].contribution_id.as_str()),
        (alpha.clone(), JOB)
    );
    // Started: the next run is a full interval away.
    f.store.plugin_schedule_started(&due[0], later).unwrap();
    assert!(f.store.plugin_due_schedules(later).unwrap().is_empty());
    // The kill switch: none are due.
    let much_later = later + chrono::Duration::minutes(6);
    f.store.plugins_set_all_off(true).unwrap();
    assert!(f.store.plugin_due_schedules(much_later).unwrap().is_empty());
    f.store.plugins_set_all_off(false).unwrap();
    assert_eq!(f.store.plugin_due_schedules(much_later).unwrap().len(), 1);

    // A success resets the count; three failures in a row pause it.
    assert!(
        !f.store
            .plugin_schedule_finished(&plugins, &due[0], "failed")
            .unwrap()
    );
    assert!(
        !f.store
            .plugin_schedule_finished(&plugins, &due[0], "done")
            .unwrap()
    );
    assert!(
        !f.store
            .plugin_schedule_finished(&plugins, &due[0], "failed")
            .unwrap()
    );
    assert!(
        !f.store
            .plugin_schedule_finished(&plugins, &due[0], "cancelled")
            .unwrap()
    );
    assert!(
        f.store
            .plugin_schedule_finished(&plugins, &due[0], "failed")
            .unwrap()
    );
    let paused = f.jobs(&alpha)[0].schedule.clone().unwrap();
    assert!(paused.paused);
    assert_eq!(paused.failures, 3);
    assert_eq!(f.last().outcome, ActivityOutcome::Failed);
    assert_eq!(
        f.last().detail,
        "keeper: paused after 3 failed runs in a row"
    );
    assert!(f.store.plugin_due_schedules(much_later).unwrap().is_empty());
    // Setting it again resumes it.
    f.schedule(&alpha, Some(10)).unwrap();
    let resumed = f.jobs(&alpha)[0].schedule.clone().unwrap();
    assert!(!resumed.paused);
    assert_eq!((resumed.failures, resumed.every_minutes), (0, 10));
    f.schedule(&alpha, None).unwrap();
    assert_eq!(f.last().detail, "keeper: off");
    assert!(f.jobs(&alpha)[0].schedule.is_none());
}

#[test]
fn disabling_revoking_trust_or_removing_deletes_schedules_and_exceptions() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    let plugins = f.plugins.clone();
    let scheduled = |f: &Fixture| {
        !f.store
            .plugin_due_schedules(chrono::Utc::now() + chrono::Duration::minutes(6))
            .unwrap()
            .is_empty()
    };

    f.enable(TrustLevel::TrustedAutomation, &alpha, &ALL);
    f.schedule(&alpha, Some(5)).unwrap();
    f.store.plugin_revoke_trusted(&plugins, KEEPER).unwrap();
    assert!(
        !scheduled(&f),
        "revoking trusted automation stops schedules"
    );

    f.enable(TrustLevel::TrustedAutomation, &alpha, &ALL);
    f.schedule(&alpha, Some(5)).unwrap();
    f.store.plugin_disable(&plugins, KEEPER).unwrap();
    assert!(!scheduled(&f), "disabling stops schedules");

    f.enable(TrustLevel::ConfirmChanges, &alpha, &ALL);
    f.schedule(&alpha, Some(5)).unwrap();
    f.store
        .plugin_set_elevation(&plugins, KEEPER, &alpha, "start-stopped", true)
        .unwrap();
    f.store.plugin_remove(&plugins, "job-plugin").unwrap();
    assert!(!scheduled(&f), "removing deletes schedules");
    assert!(
        !f.store
            .plugin_has_elevation(KEEPER, &alpha, "start-stopped")
            .unwrap(),
        "and exceptions"
    );
}

#[test]
fn a_sudo_exception_is_per_declared_action_and_server_and_logged() {
    let mut f = fixture();
    let (alpha, beta) = (f.alpha.clone(), f.beta.clone());
    let plugins = f.plugins.clone();
    f.enable(TrustLevel::ConfirmChanges, &alpha, &ALL);
    assert!(
        f.store
            .plugin_set_elevation(&plugins, KEEPER, &alpha, "undeclared", true)
            .is_err()
    );
    assert!(
        f.store
            .plugin_set_elevation(
                &plugins,
                "org.example.unknown",
                &alpha,
                "start-stopped",
                true
            )
            .is_err()
    );
    f.store
        .plugin_set_elevation(&plugins, KEEPER, &alpha, "start-stopped", true)
        .unwrap();
    assert_eq!(f.last().action, ACTION_ELEVATION_EXCEPTION);
    assert_eq!(f.last().detail, "start-stopped: granted");
    assert!(
        f.store
            .plugin_has_elevation(KEEPER, &alpha, "start-stopped")
            .unwrap()
    );
    assert!(
        !f.store
            .plugin_has_elevation(KEEPER, &alpha, "restart-worker")
            .unwrap()
    );
    assert!(
        !f.store
            .plugin_has_elevation(KEEPER, &beta, "start-stopped")
            .unwrap()
    );
    assert_eq!(f.jobs(&alpha)[0].elevation_exceptions, ["start-stopped"]);
    f.store
        .plugin_set_elevation(&plugins, KEEPER, &alpha, "start-stopped", false)
        .unwrap();
    assert_eq!(f.last().detail, "start-stopped: withdrawn");
    assert!(
        !f.store
            .plugin_has_elevation(KEEPER, &alpha, "start-stopped")
            .unwrap()
    );

    // A plugin that does not request elevate cannot be given an exception.
    let manifest = keeper_without("elevate");
    std::fs::write(
        plugins.join("job-plugin/infrawerf-plugin.json"),
        manifest.to_string(),
    )
    .unwrap();
    assert!(
        f.store
            .plugin_set_elevation(&plugins, KEEPER, &alpha, "start-stopped", true)
            .is_err()
    );
}

/// A server with three backups: one fine, one failed, one restarting.
struct BackupHost {
    asked: Vec<(String, Value)>,
}

impl SandboxHost for BackupHost {
    fn call(&mut self, function: &str, _: Capability, args: &Value) -> Result<Value, HostError> {
        self.asked.push((function.to_owned(), args.clone()));
        let service = |name: &str, scope: &str| serde_json::json!({ "id": format!("s:{name}"), "name": name, "kind": "service", "scope": scope, "runtime": "systemd", "state": "inactive" });
        match function {
            "workloads" => Ok(serde_json::json!([
                service("lab-a-backup.service", "system"),
                service("lab-b-backup.service", "system"),
                service("lab-c-backup.service", "system"),
                service("nginx.service", "system"),
                service("photos-backup-worker.service", "user"),
                { "id": "c:backup", "name": "backup", "kind": "container", "scope": "system", "runtime": "docker", "state": "running" },
            ])),
            "run_command" => {
                // As InfraWerf does: a parameter is a workload's id, never a name.
                let Some(unit) = args["parameters"]["unit"]
                    .as_str()
                    .and_then(|id| id.strip_prefix("s:"))
                else {
                    return Err(HostError::new("not_granted", "not a discovered workload"));
                };
                let value = match (args["contribution_id"].as_str().unwrap(), unit) {
                    ("last-result", "lab-b-backup.service") => "exit-code",
                    ("last-result", _) => "success",
                    ("last-finished", _) => "Wed 2026-11-18 03:00:12 SAST",
                    ("restarts", "lab-c-backup.service") => "41",
                    ("restarts", _) => "0",
                    _ => return Err(HostError::new("not_granted", "no such command")),
                };
                Ok(
                    serde_json::json!({ "result": { "outcome": "ok", "value": value, "columns": [], "rows": [], "truncated": false, "health": null } }),
                )
            }
            _ => Err(HostError::new("not_granted", "not allowed")),
        }
    }
    fn cancelled(&self) -> bool {
        false
    }
}

#[test]
fn the_systemd_backups_job_checks_each_backup_service_and_reports_the_failing_ones() {
    let manifest = validate_manifest(
        &std::fs::read(repository("examples/systemd-backups/infrawerf-plugin.json")).unwrap(),
    )
    .unwrap();
    let job = manifest
        .contributions
        .iter()
        .find_map(|c| c.job.as_ref())
        .unwrap();
    assert!(job.actions.is_empty(), "the reference plugin only reads");
    let module = std::fs::read(repository("examples/systemd-backups").join(&job.module)).unwrap();
    let mut host = BackupHost { asked: Vec::new() };
    let outcome = run(
        &module,
        &mut host,
        SandboxLimits {
            fuel: 20_000_000,
            ..SandboxLimits::default()
        },
    );
    assert_eq!(outcome.exit_code, Some(0), "{outcome:?}");
    // Only system services with "backup" in their name, each read three ways.
    let units: Vec<&str> = host
        .asked
        .iter()
        .filter(|(f, a)| f == "run_command" && a["contribution_id"] == "last-result")
        .map(|(_, a)| a["parameters"]["unit"].as_str().unwrap())
        .collect();
    assert_eq!(
        units,
        [
            "s:lab-a-backup.service",
            "s:lab-b-backup.service",
            "s:lab-c-backup.service"
        ]
    );
    assert_eq!(host.asked.len(), 1 + 3 * 3);
    assert_eq!(
        outcome.logs,
        [
            "ok lab-a-backup.service: success, last finished Wed 2026-11-18 03:00:12 SAST, 0 restarts",
            "FAILING lab-b-backup.service: exit-code, last finished Wed 2026-11-18 03:00:12 SAST, 0 restarts",
            "FAILING lab-c-backup.service: success, last finished Wed 2026-11-18 03:00:12 SAST, 41 restarts",
            "3 backups checked, 2 failing",
        ]
    );
    let result = outcome.result.unwrap();
    assert_eq!(result["failing"], 2);
    assert_eq!(result["backups"][1]["result"], "exit-code");
}
