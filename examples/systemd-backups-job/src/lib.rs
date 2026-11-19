//! The "Check backups" job of the Systemd backups example plugin.
//!
//! It runs in InfraWerf's sandbox (contract version 1, see
//! docs/plugin-api/logic.md) and reaches the server only through host
//! functions the owner allowed:
//!
//! 1. `workloads` lists the server's saved workloads; the job keeps the
//!    system services whose name contains "backup".
//! 2. For each one, `run_command` runs three of the plugin's own read-only
//!    commands with that service as `{unit}` (a parameter's value is the
//!    workload's `id`, never its name): `last-result`,
//!    `last-finished` and `restarts`.
//! 3. It logs one line per backup and sets a result the owner can read.
//!
//! The job finishes with exit code 0 even when it finds a failed backup:
//! finding a problem is the job working. It exits with 1 only when it could
//! not check at all.
//!
//! Build it from the repository root (needs `rustup target add wasm32-unknown-unknown`):
//!
//!     cargo build --release --target wasm32-unknown-unknown --manifest-path examples/systemd-backups-job/Cargo.toml
//!
//! then copy `examples/systemd-backups-job/target/wasm32-unknown-unknown/release/systemd_backups_job.wasm`
//! to `examples/systemd-backups/job.wasm`. The source lives outside the plugin
//! folder so that installing the plugin copies only what it needs.

use serde_json::{Value, json};

#[link(wasm_import_module = "infrawerf")]
unsafe extern "C" {
    fn call(ptr: i32, len: i32) -> i64;
    fn log(ptr: i32, len: i32);
    fn result(ptr: i32, len: i32);
}

/// Room for an answer from InfraWerf. The job takes the bytes back in `host`.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: i32) -> i32 {
    let room = vec![0u8; len.max(0) as usize].into_boxed_slice();
    Box::into_raw(room) as *mut u8 as i32
}

/// Calls a host function. An answer is `{"ok": ...}` or `{"error": ...}`.
fn host(function: &str, args: Value) -> Result<Value, String> {
    let request = json!({ "function": function, "args": args }).to_string();
    let packed = unsafe { call(request.as_ptr() as i32, request.len() as i32) };
    let (ptr, len) = ((packed >> 32) as u32 as usize, packed as u32 as usize);
    // SAFETY: The host returns the pointer and length from this module's
    // alloc, after writing exactly that many bytes. Recreate the boxed slice
    // with its original allocation layout and reclaim it exactly once.
    let bytes = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr as *mut u8, len)) };
    let mut answer: Value =
        serde_json::from_slice(&bytes).map_err(|_| "unreadable answer".to_owned())?;
    match answer.get_mut("ok") {
        Some(ok) => Ok(ok.take()),
        None => Err(answer["error"].as_str().unwrap_or("error").to_owned()),
    }
}

fn say(line: &str) {
    unsafe { log(line.as_ptr() as i32, line.len() as i32) }
}

fn finish(value: &Value) {
    let text = value.to_string();
    unsafe { result(text.as_ptr() as i32, text.len() as i32) }
}

/// One command's value for one backup, or why there is none.
fn read(contribution: &str, workload_id: &str) -> Result<String, String> {
    let observation = host(
        "run_command",
        json!({ "contribution_id": contribution, "parameters": { "unit": workload_id } }),
    )?;
    let outcome = observation["result"]["outcome"]
        .as_str()
        .unwrap_or("unreadable");
    match observation["result"]["value"].as_str() {
        Some(value) if outcome == "ok" => Ok(value.to_owned()),
        _ => Err(outcome.to_owned()),
    }
}

#[derive(Default)]
struct Backup {
    unit: String,
    id: String,
    result: Option<String>,
    finished: Option<String>,
    restarts: Option<String>,
}

impl Backup {
    /// Failed or looping: its last run did not succeed, or it keeps restarting.
    fn failing(&self) -> bool {
        self.result.as_deref().is_some_and(|r| r != "success")
            || self.restarts.as_deref().is_some_and(|n| n != "0")
    }
    fn line(&self) -> String {
        let field = |v: &Option<String>| {
            v.as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("not known")
                .to_owned()
        };
        format!(
            "{} {}: {}, last finished {}, {} restarts",
            if self.failing() { "FAILING" } else { "ok" },
            self.unit,
            field(&self.result),
            field(&self.finished),
            field(&self.restarts),
        )
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() -> i32 {
    let workloads = match host("workloads", json!({})) {
        Ok(Value::Array(workloads)) => workloads,
        Ok(_) => Vec::new(),
        Err(code) => {
            say(&format!("could not read this server's workloads: {code}"));
            finish(&json!({ "error": code }));
            return 1;
        }
    };
    let mut units: Vec<(String, String)> = workloads
        .iter()
        .filter(|w| w["kind"] == "service" && w["scope"] == "system")
        .filter_map(|w| Some((w["name"].as_str()?.to_owned(), w["id"].as_str()?.to_owned())))
        .filter(|(name, _)| name.contains("backup"))
        .collect();
    units.sort();
    units.dedup();
    if units.is_empty() {
        say("no system service with \"backup\" in its name in this server's saved inventory");
        finish(&json!({ "backups": [], "failing": 0 }));
        return 0;
    }

    let mut backups = Vec::new();
    for (unit, id) in units {
        let mut backup = Backup {
            unit: unit.clone(),
            id: id.clone(),
            ..Backup::default()
        };
        match read("last-result", &backup.id) {
            Ok(value) => backup.result = Some(value),
            Err(code) if code == "not_granted" || code == "cancelled" => {
                say(&format!("{unit}: {code}; stopping"));
                finish(&json!({ "error": code }));
                return 1;
            }
            Err(code) => backup.result = Some(format!("unreadable ({code})")),
        }
        backup.finished = read("last-finished", &backup.id).ok();
        backup.restarts = read("restarts", &backup.id).ok();
        say(&backup.line());
        backups.push(backup);
    }

    let failing = backups.iter().filter(|b| b.failing()).count();
    say(&format!(
        "{} backups checked, {failing} failing",
        backups.len()
    ));
    finish(&json!({
        "failing": failing,
        "backups": backups.iter().map(|b| json!({
            "unit": b.unit,
            "result": b.result,
            "last_finished": b.finished,
            "restarts": b.restarts,
            "failing": b.failing(),
        })).collect::<Vec<_>>(),
    }));
    0
}
