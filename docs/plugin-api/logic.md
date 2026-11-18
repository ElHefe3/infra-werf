# Plugin logic: the sandbox contract, version 1

A plugin's own logic is a WebAssembly module. InfraWerf runs it in the wasmi interpreter, with no system interface (no WASI), no network, no files and no clock. It can reach InfraWerf only through the host functions below, and each one is decided against what the owner allowed the plugin on the server the run is for.

A plugin declares its module as a `job` ([below](#declaring-a-job)). [`examples/service-summary`](../../examples/service-summary) is a complete job with its WAT source; the sandbox's own fixtures are in `crates/atlas-core/tests/fixtures/plugin-sandbox/`.

## Declaring a job

A contribution at the `job` point carries a `job`:

```json
{
  "id": "keeper",
  "point": "job",
  "title": "Keep services up",
  "capabilities": ["read_saved_state", "ssh_session", "control_workloads", "elevate"],
  "job": {
    "module": "job.wasm",
    "actions": [
      {
        "id": "restart-worker",
        "title": "Restart the worker",
        "control": "restart",
        "target": "service",
        "effect": "Restarts the worker so it picks up new work."
      }
    ]
  }
}
```

| Field | Rule |
| --- | --- |
| `module` | The module, a plain path inside the plugin's folder ending in `.wasm`. Its hash is part of the plugin's install record, so a changed module switches the plugin off until the owner reviews it. |
| `actions` | Optional. At most 16. Each is one workload control the job may ask for. A job with actions lists `ssh_session` and `control_workloads`; `elevate` only if an action may need sudo. |
| `id` | Lower-case letters, digits and hyphens, starting with a letter, 32 characters at most, unique in the job. |
| `title` | What the owner reads, 1 to 40 characters. |
| `control` | `start`, `stop` or `restart`. |
| `target` | `service` or `container`: the kind of workload, from the server's own inventory. |
| `effect` | What it does and why, one sentence of 1 to 200 characters, shown to the owner before enabling and with every request. |

A job appears in the Plugin data tab of every server where the owner allowed its reading capabilities. The owner runs it there or sets a schedule for it on that server, at least every 5 minutes and at most every 1440; a schedule runs while InfraWerf is open, and pauses after 3 failed runs in a row. Disabling the plugin, withdrawing trusted automation or removing it deletes its schedules.

## Actions

A job changes nothing itself. It asks InfraWerf with `control` to run one of its declared actions on a workload, and InfraWerf decides:

| The plugin's trust and the run | What happens |
| --- | --- |
| Read-only, or the owner did not allow `control_workloads` on the server | Refused. |
| Changes with confirmation, run started by the owner | InfraWerf's own dialog shows the plugin, the action, its effect and the workload; nothing happens unless the owner presses Run. |
| Changes with confirmation, scheduled run | Refused: an unattended run makes changes only under trusted automation. |
| Trusted automation | Runs at once, through the same reviewed control as the Services tab. |
| The workload's control needs sudo (a system service or system Docker) | Only with the owner's exception for that action on that server, only in a run the owner started, and only after the owner types the sudo password in InfraWerf's dialog. The password is used for that one action and never stored or shown to the plugin. |

Each action, run or refused, is in the plugin activity log.

## What a module provides

| Export | Type | Purpose |
| --- | --- | --- |
| `memory` | memory | The module's only memory, at most 16 MiB. |
| `alloc` | `(len: i32) -> i32` | Returns room for `len` bytes. InfraWerf writes each answer there. |
| `run` | `() -> i32` | The work. Its return value is the run's exit code. |

## What a module may import

Only these three, from the module `infrawerf`. A module that imports anything else, including WASI, a memory or a table, is refused before it runs.

| Import | Type | Purpose |
| --- | --- | --- |
| `call` | `(ptr: i32, len: i32) -> i64` | Calls a host function. The request at `ptr` is JSON: `{"function": "<name>", "args": {...}}`. The answer is JSON written into memory from `alloc`. The return value is `(ptr << 32) \| len` of the answer. |
| `log` | `(ptr: i32, len: i32)` | Adds a line of text to the run's log. |
| `result` | `(ptr: i32, len: i32)` | Sets the run's result, which must be JSON. |

An answer is `{"ok": <value>}` or `{"error": "<code>", "message": "<sentence>"}`. Error codes: `not_granted`, `unknown_function`, `unknown_server`, `bad_request`, `too_large`, `unavailable`, and for `control` also `unknown_action`, `unknown_workload`, `ambiguous_workload`, `wrong_target`, `needs_confirmation`, `no_elevation_exception`, `sudo_never_unattended`, `declined`, `no_answer` (5 minutes passed), `no_password`, `cancelled` and `failed`, or the permission refusal that applies, such as `trust_too_low`, `not_granted` or `plugin_disabled`.

## Host functions

Each host function needs exactly one capability on the server the run is for.

| Function | Needs | Answer |
| --- | --- | --- |
| `server` | `read_saved_state` | The server's `id`, `name` and `host`, as saved on this PC. |
| `inventory` | `read_saved_state` | The server's saved inventory, or `null`. |
| `workloads` | `read_saved_state` | The server's workloads from its saved inventory: each one's `id`, `name`, `kind`, `scope`, `runtime` and `state`. |
| `run_command` | `run_approved_command` | Runs one of the plugin's own pack commands on the server: `{"contribution_id": "<id>", "parameters": {"<name>": "<workload id>"}}`. A parameter's value is the `id` of a workload from `workloads`, never its name, and must be one the server's authenticated discovery found. The answer is the command's observation: its `result` holds the `outcome` and the `value`, or the `rows` of a table. |
| `control` | `control_workloads` | Asks for one declared action: `{"action": "<id>", "workload": "<name>"}` or `"workload_id"` instead of a name. See [actions](#actions). The answer is the observed outcome. |

Each call is decided against the owner's permissions at that moment and is in the plugin activity log, allowed or refused.

## Limits

Each run has these limits. None of them depends on the module's cooperation.

| Limit | Value | When reached |
| --- | --- | --- |
| Fuel (about one unit per instruction) | 50 million | `out_of_fuel` |
| Memory | 16 MiB | `memory_limit` |
| Recursion depth | 256 | `stack_limit` |
| Host calls (`call`, `log` and `result` together) | 200 | `call_limit` |
| Request size | 64 KiB | `call_limit` |
| Answer size | 256 KiB | the call answers `too_large` |
| Result size | 64 KiB | `call_limit` |
| Log | 50 lines of 300 characters | further lines are dropped |
| Module size | 4 MiB | `invalid_module` |

A run also stops with:

| Stop | Cause |
| --- | --- |
| `invalid_module` | The file is not valid WebAssembly. |
| `forbidden_import` | It imports anything outside the contract. |
| `contract_violation` | A pointer outside its memory, an `alloc` that returns memory the answer cannot fit in, or a missing export. |
| `cancelled` | Its permissions changed while it ran. |
| `trapped` | `unreachable`, division by zero and the like. |

Every run gets its own thread, so a runaway stops without affecting the rest of InfraWerf.
