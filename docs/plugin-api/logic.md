# Plugin logic: the sandbox contract, version 1

A plugin's own logic is a WebAssembly module. InfraWerf runs it in the wasmi interpreter, with no system interface (no WASI), no network, no files and no clock. It can reach InfraWerf only through the host functions below, and each one is decided against what the owner allowed the plugin on the server the run is for.

How a plugin declares its module, and how runs are started, is not part of this version. Today the sandbox is proven by InfraWerf's own tests, with fixtures in `crates/atlas-core/tests/fixtures/plugin-sandbox/` (`prototype.wat` is a complete example).

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

An answer is `{"ok": <value>}` or `{"error": "<code>", "message": "<sentence>"}`. Error codes: `not_granted`, `unknown_function`, `unknown_server`, `bad_request`, `too_large`, `unavailable`.

## Host functions

Each host function needs exactly one capability on the server the run is for.

| Function | Needs | Answer |
| --- | --- | --- |
| `server` | `read_saved_state` | The server's `id`, `name` and `host`, as saved on this PC. |
| `inventory` | `read_saved_state` | The server's saved inventory, or `null`. |

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
