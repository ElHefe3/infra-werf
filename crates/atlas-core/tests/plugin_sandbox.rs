//! The plugin sandbox (T048): a prototype reads through granted host
//! functions and is refused ungranted ones; every runaway and escape fixture
//! is stopped with its own reason, quickly, without touching the app.
use atlas_core::extensions::Capability;
use atlas_core::plugin_host::*;
use atlas_core::plugin_sandbox::*;
use atlas_core::*;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin-sandbox")
}

fn module(name: &str) -> Vec<u8> {
    let path = fixtures().join(name);
    if name.ends_with(".wat") {
        wat::parse_file(&path).unwrap_or_else(|error| panic!("{name}: {error}"))
    } else {
        std::fs::read(path).unwrap()
    }
}

/// A host that allows the listed capabilities and can cancel after some calls.
struct FakeHost {
    granted: Vec<Capability>,
    calls: std::cell::Cell<u32>,
    /// Calls that reached `call`: the host was asked to answer.
    answered: std::cell::Cell<u32>,
    cancel_after: Option<u32>,
}

impl FakeHost {
    fn allowing(granted: &[Capability]) -> Self {
        Self {
            granted: granted.to_vec(),
            calls: std::cell::Cell::new(0),
            answered: std::cell::Cell::new(0),
            cancel_after: None,
        }
    }
}

impl SandboxHost for FakeHost {
    fn call(
        &mut self,
        function: &str,
        capability: Capability,
        _: &Value,
    ) -> Result<Value, HostError> {
        self.answered.set(self.answered.get() + 1);
        if !self.granted.contains(&capability) {
            return Err(HostError::new("not_granted", "not allowed"));
        }
        Ok(serde_json::json!({ "function": function, "name": "Alpha" }))
    }
    fn cancelled(&self) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.cancel_after
            .is_some_and(|after| self.calls.get() > after)
    }
}

/// Test limits: the defaults with less fuel, as tests run as an unoptimised
/// build where the interpreter is far slower (a release build spends the
/// default 50 million in well under a second).
fn limits() -> SandboxLimits {
    SandboxLimits {
        fuel: 2_000_000,
        ..SandboxLimits::default()
    }
}

fn run_fixture(name: &str, host: &mut (dyn SandboxHost + Send)) -> SandboxOutcome {
    run(&module(name), host, limits())
}

#[test]
fn the_prototype_reads_through_granted_functions_and_is_refused_the_rest() {
    let mut host = FakeHost::allowing(&[Capability::ReadSavedState]);
    let outcome = run_fixture("prototype.wat", &mut host);
    assert_eq!(outcome.stopped, None, "{:?}", outcome.reason);
    assert_eq!(outcome.exit_code, Some(0));
    assert_eq!(
        outcome.logs[0],
        "prototype: reading the server and its inventory"
    );
    assert_eq!(
        outcome.logs[1],
        r#"{"ok":{"function":"server","name":"Alpha"}}"#
    );
    assert_eq!(
        outcome.logs[2],
        r#"{"ok":{"function":"inventory","name":"Alpha"}}"#
    );
    assert_eq!(
        outcome.result,
        Some(serde_json::json!({ "ok": { "function": "server", "name": "Alpha" } }))
    );
    // log, call, log, call, log, result
    assert_eq!(outcome.host_calls, 6);
    assert!(outcome.refused.is_empty());
    // Not granted: the module gets an answer it can read, nothing else.
    let mut host = FakeHost::allowing(&[]);
    let outcome = run_fixture("prototype.wat", &mut host);
    assert_eq!(outcome.exit_code, Some(0));
    assert!(
        outcome.logs[1].starts_with(r#"{"error":"not_granted""#),
        "{}",
        outcome.logs[1]
    );
    assert_eq!(outcome.refused, ["server", "inventory"]);
    // A function outside the contract is answered as unknown; the host is
    // never asked.
    let mut host = FakeHost::allowing(&[Capability::ReadSavedState]);
    let outcome = run_fixture("unknown_function.wat", &mut host);
    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.logs[0].starts_with(r#"{"error":"unknown_function""#));
    assert_eq!(host.answered.get(), 0, "the host was never asked");
}

#[test]
fn every_runaway_and_escape_is_stopped_with_its_own_reason_quickly() {
    let expected = [
        ("out_of_fuel.wat", SandboxStop::OutOfFuel),
        ("start_loop.wat", SandboxStop::OutOfFuel),
        ("memory_limit.wat", SandboxStop::MemoryLimit),
        ("memory_limit_initial.wat", SandboxStop::MemoryLimit),
        ("stack_limit.wat", SandboxStop::StackLimit),
        ("call_limit.wat", SandboxStop::CallLimit),
        ("call_limit_request.wat", SandboxStop::CallLimit),
        (
            "contract_violation_pointer.wat",
            SandboxStop::ContractViolation,
        ),
        (
            "contract_violation_alloc.wat",
            SandboxStop::ContractViolation,
        ),
        (
            "contract_violation_no_run.wat",
            SandboxStop::ContractViolation,
        ),
        ("forbidden_import_wasi.wat", SandboxStop::ForbiddenImport),
        ("forbidden_import_memory.wat", SandboxStop::ForbiddenImport),
        (
            "forbidden_import_function.wat",
            SandboxStop::ForbiddenImport,
        ),
        ("trapped.wat", SandboxStop::Trapped),
        ("invalid_module.wasm", SandboxStop::InvalidModule),
    ];
    let mut checked = std::collections::BTreeSet::new();
    for (name, stop) in expected {
        let started = Instant::now();
        let mut host = FakeHost::allowing(&[Capability::ReadSavedState]);
        let outcome = run_fixture(name, &mut host);
        let took = started.elapsed();
        assert_eq!(outcome.stopped, Some(stop), "{name}: {:?}", outcome.reason);
        assert_eq!(outcome.exit_code, None, "{name}");
        assert!(
            outcome.reason.as_deref().is_some_and(|r| !r.is_empty()),
            "{name}"
        );
        assert!(took < Duration::from_secs(10), "{name} took {took:?}");
        checked.insert(name.split('.').next().unwrap().to_owned());
    }
    // A run whose permissions go away while it runs stops as cancelled.
    let mut host = FakeHost::allowing(&[]);
    host.cancel_after = Some(10);
    let outcome = run_fixture("cancelled.wat", &mut host);
    assert_eq!(outcome.stopped, Some(SandboxStop::Cancelled));
    checked.insert("cancelled".into());
    // Every fixture file is covered.
    let files: std::collections::BTreeSet<String> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .split('.')
                .next()
                .unwrap()
                .to_owned()
        })
        .filter(|name| name != "prototype" && name != "unknown_function")
        .collect();
    assert_eq!(checked, files);
}

#[test]
fn a_runaway_does_not_hold_up_other_work() {
    // A runaway on its own thread; the app's other work, here a second run,
    // completes meanwhile, and the runaway ends on its own.
    let runaway = std::thread::spawn(|| {
        let mut host = FakeHost::allowing(&[]);
        let started = Instant::now();
        let outcome = run(&module("out_of_fuel.wat"), &mut host, limits());
        (outcome.stopped, started.elapsed())
    });
    let started = Instant::now();
    let mut host = FakeHost::allowing(&[Capability::ReadSavedState]);
    let outcome = run(&module("prototype.wat"), &mut host, limits());
    assert_eq!(outcome.exit_code, Some(0));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    let (stopped, took) = runaway.join().expect("the runaway thread did not panic");
    assert_eq!(stopped, Some(SandboxStop::OutOfFuel));
    assert!(took < Duration::from_secs(10), "{took:?}");
    // Limits are the run's own: a smaller fuel budget stops sooner.
    let mut host = FakeHost::allowing(&[]);
    let small = SandboxLimits {
        fuel: 1000,
        ..SandboxLimits::default()
    };
    let outcome = run(&module("out_of_fuel.wat"), &mut host, small);
    assert_eq!(outcome.stopped, Some(SandboxStop::OutOfFuel));
    assert!(outcome.fuel_used <= 1000);
}

#[test]
fn the_store_host_decides_each_call_against_the_owners_grants_and_logs_it() {
    let directory = tempfile::tempdir().unwrap();
    let plugins = directory.path().join("plugins");
    std::fs::create_dir_all(plugins.join("logic")).unwrap();
    std::fs::write(
        plugins.join("logic/infrawerf-plugin.json"),
        serde_json::json!({
            "id": "org.example.logic", "name": "Logic", "version": "1.0.0", "api_version": 2,
            "capabilities": ["read_saved_state"],
            "contributions": [{ "id": "badge", "point": "server_badge", "title": "Logic", "capabilities": ["read_saved_state"] }]
        })
        .to_string(),
    )
    .unwrap();
    let mut store = Store::create(&directory.path().join("atlas.sqlite3")).unwrap();
    let server = store
        .add_server(AddServer {
            name: "Alpha".into(),
            host: "alpha.invalid".into(),
        })
        .unwrap();
    let wasm = module("prototype.wat");
    let go = |store: &mut Store| {
        let mut host = StoreHost {
            store,
            directory: &plugins,
            plugin: "org.example.logic".into(),
            server: server.clone(),
            cancel: None,
        };
        run(&wasm, &mut host, limits())
    };
    // Not enabled: refused, and each refusal is logged.
    let outcome = go(&mut store);
    assert_eq!(outcome.refused, ["server", "inventory"]);
    // Enabled and allowed on the server: the real server comes back.
    store
        .plugin_enable(&plugins, "org.example.logic", TrustLevel::ReadOnly)
        .unwrap();
    store
        .plugin_set_grant(
            &plugins,
            "org.example.logic",
            &server,
            &[Capability::ReadSavedState],
        )
        .unwrap();
    let outcome = go(&mut store);
    assert!(outcome.refused.is_empty());
    let result = outcome.result.unwrap();
    assert_eq!(result["ok"]["name"], "Alpha");
    assert_eq!(result["ok"]["host"], "alpha.invalid");
    let log = store.plugin_activity(50).unwrap().entries;
    let decisions: Vec<(String, ActivityOutcome)> = log
        .iter()
        .filter(|e| e.action == "read_saved_state")
        .map(|e| (e.detail.clone(), e.outcome))
        .collect();
    assert_eq!(
        decisions
            .iter()
            .filter(|(_, o)| *o == ActivityOutcome::Allowed)
            .count(),
        2
    );
    assert_eq!(
        decisions
            .iter()
            .filter(|(_, o)| *o == ActivityOutcome::Refused)
            .count(),
        2
    );
    // Turning all plugins off refuses the next run at once.
    store.plugins_set_all_off(true).unwrap();
    assert_eq!(go(&mut store).refused, ["server", "inventory"]);
}

#[test]
fn each_host_function_maps_to_exactly_one_capability() {
    for (name, capability, summary) in HOST_FUNCTIONS {
        assert_eq!(capability_for(name), Some(*capability));
        assert!(!summary.is_empty());
    }
    assert_eq!(capability_for("run_shell"), None);
    assert_eq!(SANDBOX_ABI_VERSION, 1);
}

#[test]
fn the_contract_page_documents_every_function_stop_and_limit() {
    let page = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/plugin-api/logic.md"),
    )
    .unwrap();
    for (name, capability, _) in HOST_FUNCTIONS {
        let capability = serde_json::to_value(capability).unwrap();
        assert!(
            page.contains(&format!(
                "| `{name}` | `{}` |",
                capability.as_str().unwrap()
            )),
            "{name}"
        );
    }
    for stop in [
        SandboxStop::InvalidModule,
        SandboxStop::ForbiddenImport,
        SandboxStop::OutOfFuel,
        SandboxStop::MemoryLimit,
        SandboxStop::StackLimit,
        SandboxStop::CallLimit,
        SandboxStop::ContractViolation,
        SandboxStop::Cancelled,
        SandboxStop::Trapped,
    ] {
        let code = serde_json::to_value(stop).unwrap();
        assert!(
            page.contains(&format!("`{}`", code.as_str().unwrap())),
            "{code}"
        );
    }
    let limits = SandboxLimits::default();
    for text in [
        "50 million".to_owned(),
        format!("{} MiB", limits.memory_bytes / 1024 / 1024),
        format!("| {} |", limits.recursion_depth),
        format!("| {} |", limits.host_calls),
        format!("{} KiB", limits.request_bytes / 1024),
        format!("{} KiB", limits.answer_bytes / 1024),
        format!(
            "{} lines of {} characters",
            limits.log_lines, limits.log_line_chars
        ),
        format!("{} MiB", limits.module_bytes / 1024 / 1024),
    ] {
        assert!(page.contains(&text), "{text}");
    }
    assert_eq!(limits.fuel, 50_000_000);
}
