//! Sandboxed plugin logic (S12/F24/T048).
//!
//! A plugin may ship logic as a WebAssembly module. It runs in the wasmi
//! interpreter (owner decision D060): no just-in-time code, no system
//! interface (no WASI), and no imports except the two host functions below.
//! It can reach InfraWerf only through `infrawerf.call`, and every call names
//! a host function that maps to exactly one capability; the host decides it
//! against the owner's grants at that moment. Fuel, memory, stack, host
//! calls and output are all bounded, so a runaway module stops with a clear
//! reason and leaves the app untouched.
//!
//! The module's side of the contract (version 1):
//!
//! - exports `memory`, `alloc(len: i32) -> i32` and `run() -> i32`;
//! - may import `infrawerf.call(ptr: i32, len: i32) -> i64`: the request is
//!   JSON `{"function": "...", "args": {...}}`; the answer is written into
//!   memory the host obtains from `alloc`, and the return value is
//!   `(ptr << 32) | len`. The answer is `{"ok": ...}` or
//!   `{"error": "<code>", "message": "..."}`;
//! - may import `infrawerf.log(ptr: i32, len: i32)` to add a line to the
//!   run's log, and `infrawerf.result(ptr: i32, len: i32)` to set the run's
//!   JSON result.
use crate::extensions::Capability;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::{Config, TS};
use wasmi::{
    Caller, Config as EngineConfig, Engine, Extern, Linker, Memory, Module, Store, StoreLimits,
    StoreLimitsBuilder, TrapCode,
};

/// The version of the module contract above.
pub const SANDBOX_ABI_VERSION: u32 = 1;
pub const HOST_MODULE: &str = "infrawerf";

/// The first host-function set. Each maps one-to-one onto the capability
/// the owner must have granted on the server the run is for.
pub const HOST_FUNCTIONS: &[(&str, Capability, &str)] = &[
    (
        "server",
        Capability::ReadSavedState,
        "The server's ID, name and host, as saved on this PC.",
    ),
    (
        "inventory",
        Capability::ReadSavedState,
        "The server's saved inventory, or null.",
    ),
    (
        "workloads",
        Capability::ReadSavedState,
        "The server's discovered services and containers: id, name, kind, scope and state.",
    ),
    (
        "run_command",
        Capability::RunApprovedCommand,
        "Runs one of this plugin's own read-only pack commands on the server and returns its observation. Args: contribution_id, and parameters as workload IDs.",
    ),
    (
        "control",
        Capability::ControlWorkloads,
        "Asks for one of this job's declared actions on a workload. Args: action, and workload_id from workloads. InfraWerf asks the owner first unless the plugin is at trusted automation.",
    ),
];

/// The capability a host function needs, if it exists.
pub fn capability_for(function: &str) -> Option<Capability> {
    HOST_FUNCTIONS
        .iter()
        .find(|(name, _, _)| *name == function)
        .map(|(_, capability, _)| *capability)
}

/// Bounds on one run. All of them are enforced by the runtime or the host
/// functions; none relies on the module's cooperation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxLimits {
    /// Fuel: roughly one unit per instruction. Bounds compute time.
    pub fuel: u64,
    /// Largest linear memory, in bytes.
    pub memory_bytes: usize,
    /// Deepest call recursion.
    pub recursion_depth: usize,
    /// Host calls per run (`call`, `log` and `result` together).
    pub host_calls: u32,
    /// Largest request a module may pass to a host function.
    pub request_bytes: usize,
    /// Largest answer a host function may return.
    pub answer_bytes: usize,
    /// Log lines kept, and characters per line.
    pub log_lines: usize,
    pub log_line_chars: usize,
    /// Largest result.
    pub result_bytes: usize,
    /// Largest module file.
    pub module_bytes: usize,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            fuel: 50_000_000,
            memory_bytes: 16 * 1024 * 1024,
            recursion_depth: 256,
            host_calls: 200,
            request_bytes: 64 * 1024,
            answer_bytes: 256 * 1024,
            log_lines: 50,
            log_line_chars: 300,
            result_bytes: 64 * 1024,
            module_bytes: 4 * 1024 * 1024,
        }
    }
}

/// A host function's refusal or failure, returned to the module as data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostError {
    pub code: String,
    pub message: String,
}

impl HostError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// InfraWerf's side of a run: decides each call against the owner's grants
/// for one plugin on one server. Implemented by the core (`StoreHost`) and
/// by tests.
pub trait SandboxHost {
    /// Answers a known host function. `capability` is the one it maps to;
    /// the host must check it before doing anything.
    fn call(
        &mut self,
        function: &str,
        capability: Capability,
        args: &Value,
    ) -> Result<Value, HostError>;
    /// The run must stop now (permissions changed, the app is closing).
    fn cancelled(&self) -> bool {
        false
    }
}

/// Why a run ended without completing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStop {
    /// The module is malformed, too large, or does not meet the contract.
    InvalidModule,
    /// The module imports something other than InfraWerf's host functions.
    ForbiddenImport,
    /// It used all its fuel.
    OutOfFuel,
    /// It asked for more memory than allowed.
    MemoryLimit,
    /// It recursed too deeply.
    StackLimit,
    /// It made more host calls than allowed, or passed a request or result
    /// that is too large.
    CallLimit,
    /// It passed the host something outside its memory, or broke the
    /// contract in another way.
    ContractViolation,
    /// Its permissions changed while it ran, or InfraWerf is closing.
    Cancelled,
    /// It trapped: unreachable code, division by zero and the like.
    Trapped,
}

/// The outcome of one run, for the activity log and the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SandboxOutcome {
    /// `run`'s return value, when it completed.
    pub exit_code: Option<i32>,
    pub stopped: Option<SandboxStop>,
    /// A sentence about why it stopped; never module output.
    pub reason: Option<String>,
    pub logs: Vec<String>,
    #[ts(type = "unknown")]
    pub result: Option<Value>,
    #[ts(type = "number")]
    pub fuel_used: u64,
    pub host_calls: u32,
    /// Host functions refused because the owner did not allow them.
    pub refused: Vec<String>,
}

impl SandboxOutcome {
    fn stopped(stop: SandboxStop, reason: impl Into<String>) -> Self {
        Self {
            exit_code: None,
            stopped: Some(stop),
            reason: Some(reason.into()),
            logs: Vec::new(),
            result: None,
            fuel_used: 0,
            host_calls: 0,
            refused: Vec::new(),
        }
    }
}

/// What the store carries during a run.
struct RunState<'h> {
    host: &'h mut (dyn SandboxHost + Send),
    limits: SandboxLimits,
    store_limits: StoreLimits,
    calls: u32,
    logs: Vec<String>,
    result: Option<Value>,
    refused: Vec<String>,
    /// Set by a host function that ended the run, so the cause is not
    /// mistaken for a plain trap.
    stop: Option<(SandboxStop, String)>,
}

fn host_trap(
    caller: &mut Caller<'_, RunState<'_>>,
    stop: SandboxStop,
    reason: &str,
) -> wasmi::Error {
    caller.data_mut().stop = Some((stop, reason.into()));
    wasmi::Error::new(reason.to_owned())
}

fn memory(caller: &Caller<'_, RunState<'_>>) -> Option<Memory> {
    match caller.get_export("memory") {
        Some(Extern::Memory(memory)) => Some(memory),
        _ => None,
    }
}

/// Copies `len` bytes at `ptr` out of the module's memory, or traps.
fn read_bytes(
    caller: &mut Caller<'_, RunState<'_>>,
    ptr: i32,
    len: i32,
    limit: usize,
) -> Result<Vec<u8>, wasmi::Error> {
    if len < 0 || ptr < 0 {
        return Err(host_trap(
            caller,
            SandboxStop::ContractViolation,
            "A pointer or length is negative.",
        ));
    }
    if len as usize > limit {
        return Err(host_trap(
            caller,
            SandboxStop::CallLimit,
            "A request is larger than allowed.",
        ));
    }
    let Some(memory) = memory(caller) else {
        return Err(host_trap(
            caller,
            SandboxStop::ContractViolation,
            "The module exports no memory.",
        ));
    };
    let mut buffer = vec![0; len as usize];
    if memory.read(&*caller, ptr as usize, &mut buffer).is_err() {
        return Err(host_trap(
            caller,
            SandboxStop::ContractViolation,
            "A request lies outside the module's memory.",
        ));
    }
    Ok(buffer)
}

fn count_call(caller: &mut Caller<'_, RunState<'_>>) -> Result<(), wasmi::Error> {
    if caller.data().host.cancelled() {
        return Err(host_trap(
            caller,
            SandboxStop::Cancelled,
            "The run was cancelled.",
        ));
    }
    let state = caller.data_mut();
    state.calls += 1;
    if state.calls > state.limits.host_calls {
        return Err(host_trap(
            caller,
            SandboxStop::CallLimit,
            "The module made more host calls than allowed.",
        ));
    }
    Ok(())
}

/// Writes `bytes` into memory the module allocates, returning `(ptr << 32) | len`.
fn write_answer(caller: &mut Caller<'_, RunState<'_>>, bytes: &[u8]) -> Result<i64, wasmi::Error> {
    let alloc = match caller.get_export("alloc") {
        Some(Extern::Func(func)) => func.typed::<i32, i32>(&*caller).ok(),
        _ => None,
    };
    let Some(alloc) = alloc else {
        return Err(host_trap(
            caller,
            SandboxStop::ContractViolation,
            "The module exports no alloc(len) -> ptr.",
        ));
    };
    let ptr = alloc.call(&mut *caller, bytes.len() as i32)?;
    let Some(memory) = memory(caller) else {
        return Err(host_trap(
            caller,
            SandboxStop::ContractViolation,
            "The module exports no memory.",
        ));
    };
    if ptr < 0 || memory.write(&mut *caller, ptr as usize, bytes).is_err() {
        return Err(host_trap(
            caller,
            SandboxStop::ContractViolation,
            "alloc returned memory the answer does not fit in.",
        ));
    }
    Ok(((ptr as i64) << 32) | bytes.len() as i64)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    function: String,
    #[serde(default)]
    args: Value,
}

fn link(linker: &mut Linker<RunState<'_>>) -> Result<(), wasmi::Error> {
    linker.func_wrap(
        HOST_MODULE,
        "call",
        |mut caller: Caller<'_, RunState<'_>>, ptr: i32, len: i32| -> Result<i64, wasmi::Error> {
            count_call(&mut caller)?;
            let limit = caller.data().limits.request_bytes;
            let bytes = read_bytes(&mut caller, ptr, len, limit)?;
            let answer = match serde_json::from_slice::<Request>(&bytes) {
                Err(_) => serde_json::json!({ "error": "bad_request", "message": "Send {\"function\": \"...\", \"args\": {...}}." }),
                Ok(request) => match capability_for(&request.function) {
                    None => serde_json::json!({ "error": "unknown_function", "message": "There is no such host function in this contract version." }),
                    Some(capability) => {
                        let state = caller.data_mut();
                        match state.host.call(&request.function, capability, &request.args) {
                            Ok(value) => serde_json::json!({ "ok": value }),
                            Err(error) => {
                                if error.code == "not_granted" {
                                    state.refused.push(request.function.clone());
                                }
                                serde_json::json!({ "error": error.code, "message": error.message })
                            }
                        }
                    }
                },
            };
            let bytes = serde_json::to_vec(&answer).unwrap_or_default();
            if bytes.len() > caller.data().limits.answer_bytes {
                let small = br#"{"error":"too_large","message":"The answer is larger than a module may receive."}"#;
                return write_answer(&mut caller, small);
            }
            write_answer(&mut caller, &bytes)
        },
    )?;
    linker.func_wrap(
        HOST_MODULE,
        "log",
        |mut caller: Caller<'_, RunState<'_>>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
            count_call(&mut caller)?;
            let limit = caller.data().limits.request_bytes;
            let bytes = read_bytes(&mut caller, ptr, len, limit)?;
            let state = caller.data_mut();
            if state.logs.len() < state.limits.log_lines {
                let line: String = String::from_utf8_lossy(&bytes)
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(state.limits.log_line_chars)
                    .collect();
                state.logs.push(line);
            }
            Ok(())
        },
    )?;
    linker.func_wrap(
        HOST_MODULE,
        "result",
        |mut caller: Caller<'_, RunState<'_>>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
            count_call(&mut caller)?;
            let limit = caller.data().limits.result_bytes;
            let bytes = read_bytes(&mut caller, ptr, len, limit)?;
            let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
                return Err(host_trap(
                    &mut caller,
                    SandboxStop::ContractViolation,
                    "The result is not JSON.",
                ));
            };
            caller.data_mut().result = Some(value);
            Ok(())
        },
    )?;
    Ok(())
}

fn engine(limits: &SandboxLimits) -> Engine {
    let mut config = EngineConfig::default();
    config
        .consume_fuel(true)
        .set_max_recursion_depth(limits.recursion_depth)
        // Less surface: SIMD, multi-memory and memory64 are not compiled
        // into this build of wasmi at all; custom page sizes are off.
        .wasm_custom_page_sizes(false);
    Engine::new(&config)
}

/// The native stack the interpreter gets. A module's own recursion is bounded
/// by `recursion_depth`; this keeps the interpreter itself, and the host
/// functions it calls back into, far from the end of the stack whatever
/// thread the caller is on (a native overflow would end the whole app).
pub const SANDBOX_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Runs a module's `run` export for one plugin on one server, on a thread of
/// its own with a fixed stack. The caller waits for it.
pub fn run(
    module: &[u8],
    host: &mut (dyn SandboxHost + Send),
    limits: SandboxLimits,
) -> SandboxOutcome {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("plugin-sandbox".into())
            .stack_size(SANDBOX_STACK_BYTES)
            .spawn_scoped(scope, || run_here(module, host, limits))
            .map_err(|_| ())
            .and_then(|thread| thread.join().map_err(|_| ()))
            .unwrap_or_else(|()| {
                SandboxOutcome::stopped(SandboxStop::Trapped, "The sandbox thread failed.")
            })
    })
}

fn run_here(
    module: &[u8],
    host: &mut (dyn SandboxHost + Send),
    limits: SandboxLimits,
) -> SandboxOutcome {
    if module.len() > limits.module_bytes {
        return SandboxOutcome::stopped(
            SandboxStop::InvalidModule,
            "The module is larger than allowed.",
        );
    }
    let engine = engine(&limits);
    let module = match Module::new(&engine, module) {
        Ok(module) => module,
        Err(_) => {
            return SandboxOutcome::stopped(
                SandboxStop::InvalidModule,
                "The file is not a valid WebAssembly module.",
            );
        }
    };
    // Only InfraWerf's own host functions may be imported: no system
    // interface, no memory or table from outside.
    for import in module.imports() {
        let allowed =
            import.module() == HOST_MODULE && matches!(import.name(), "call" | "log" | "result");
        if !allowed {
            return SandboxOutcome::stopped(
                SandboxStop::ForbiddenImport,
                format!(
                    "The module imports {}.{}; only infrawerf.call, infrawerf.log and infrawerf.result are available.",
                    import.module().chars().take(40).collect::<String>(),
                    import.name().chars().take(40).collect::<String>()
                ),
            );
        }
    }
    let store_limits = StoreLimitsBuilder::new()
        .memory_size(limits.memory_bytes)
        .memories(1)
        .tables(1)
        .table_elements(10_000)
        .instances(1)
        .trap_on_grow_failure(true)
        .build();
    let state = RunState {
        host,
        limits,
        store_limits,
        calls: 0,
        logs: Vec::new(),
        result: None,
        refused: Vec::new(),
        stop: None,
    };
    let mut store = Store::new(&engine, state);
    store.limiter(|state| &mut state.store_limits);
    if store.set_fuel(limits.fuel).is_err() {
        return SandboxOutcome::stopped(SandboxStop::InvalidModule, "Fuel could not be set.");
    }
    let mut linker = Linker::new(&engine);
    if link(&mut linker).is_err() {
        return SandboxOutcome::stopped(
            SandboxStop::InvalidModule,
            "The host functions could not be linked.",
        );
    }
    let outcome = linker
        .instantiate_and_start(&mut store, &module)
        .and_then(|instance| {
            let run = instance
                .get_typed_func::<(), i32>(&store, "run")
                .map_err(|_| wasmi::Error::new("The module exports no run() -> i32."))?;
            if instance.get_memory(&store, "memory").is_none() {
                return Err(wasmi::Error::new("The module exports no memory."));
            }
            run.call(&mut store, ())
        });
    let fuel_used = limits.fuel.saturating_sub(store.get_fuel().unwrap_or(0));
    let state = store.data_mut();
    let mut result = SandboxOutcome {
        exit_code: None,
        stopped: None,
        reason: None,
        logs: std::mem::take(&mut state.logs),
        result: state.result.take(),
        fuel_used,
        host_calls: state.calls,
        refused: std::mem::take(&mut state.refused),
    };
    match outcome {
        Ok(code) => result.exit_code = Some(code),
        Err(error) => {
            let (stop, reason) = match (state.stop.take(), error.as_trap_code()) {
                (Some(stop), _) => stop,
                (None, Some(TrapCode::OutOfFuel)) => (
                    SandboxStop::OutOfFuel,
                    "The module used all its fuel.".into(),
                ),
                (None, Some(TrapCode::StackOverflow)) => (
                    SandboxStop::StackLimit,
                    "The module recursed too deeply.".into(),
                ),
                (None, Some(TrapCode::GrowthOperationLimited)) => (
                    SandboxStop::MemoryLimit,
                    "The module asked for more memory than allowed.".into(),
                ),
                (None, Some(code)) => {
                    (SandboxStop::Trapped, format!("The module trapped: {code}."))
                }
                (None, None) => {
                    let text = error.to_string();
                    if text.contains("memory")
                        && (text.contains("limit") || text.contains("resource"))
                    {
                        (
                            SandboxStop::MemoryLimit,
                            "The module asked for more memory than allowed.".into(),
                        )
                    } else {
                        (
                            SandboxStop::ContractViolation,
                            text.chars().take(200).collect(),
                        )
                    }
                }
            };
            result.stopped = Some(stop);
            result.reason = Some(reason);
        }
    }
    result
}

pub fn typescript_contracts(config: &Config) -> String {
    [SandboxStop::decl(config), SandboxOutcome::decl(config)]
        .into_iter()
        .map(|d| format!("export {d}\n"))
        .collect()
}

/// The host for a run of a plugin installed in `directory`, on one server:
/// each call is decided by `Store::plugin_authorize`, exactly like any
/// other plugin call, and logged there.
pub struct StoreHost<'a> {
    pub store: &'a mut crate::Store,
    pub directory: &'a std::path::Path,
    pub plugin: String,
    pub server: crate::ServerId,
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl SandboxHost for StoreHost<'_> {
    fn call(
        &mut self,
        function: &str,
        capability: Capability,
        _args: &Value,
    ) -> Result<Value, HostError> {
        let decision = self
            .store
            .plugin_authorize(self.directory, &self.plugin, &self.server, capability)
            .map_err(|_| HostError::new("unavailable", "InfraWerf could not decide this call."))?;
        if !decision.allowed || decision.needs_confirmation {
            return Err(HostError::new(
                "not_granted",
                "The owner has not allowed this plugin to do that on this server.",
            ));
        }
        let server = self
            .store
            .snapshot()
            .ok()
            .and_then(|state| state.servers.into_iter().find(|s| s.id == self.server))
            .ok_or_else(|| {
                HostError::new("unknown_server", "The server is no longer on the map.")
            })?;
        match function {
            "server" => {
                Ok(serde_json::json!({ "id": server.id, "name": server.name, "host": server.host }))
            }
            "inventory" => Ok(serde_json::to_value(&server.inventory).unwrap_or(Value::Null)),
            _ => Err(HostError::new(
                "unknown_function",
                "There is no such host function.",
            )),
        }
    }
    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst))
    }
}
