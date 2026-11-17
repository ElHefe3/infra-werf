# T048 acceptance — sandbox runtime and host functions

Date: 2026-11-17. Feature: F24 / S12. Ticket: T048.
Revision: 7c49d10
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64. The sandbox is exercised by core tests in both an unoptimised and a release build. The application does not call it yet (T049).

## What was built

- **The decision.** Owner decision [D060](../discovery/D060-plugin-sandbox-wasmi.md): plugin logic is a WebAssembly module run in the wasmi interpreter. The note compares wasmi, wasmtime and Extism on how they run, executable memory, Android, size, limits, speed, author experience and audit surface.
- **The sandbox** (`crates/atlas-core/src/plugin_sandbox.rs`), contract version 1:
  - A module exports `memory`, `alloc` and `run`. It may import only `infrawerf.call`, `log` and `result`.
  - Requests and answers are JSON in the module's memory. Pointers and sizes are checked, and the module traps rather than the host reading or writing outside its memory.
  - wasmi `=2.0.0` is built with only `std`, `validate` and `auto-dispatch`.
- **Host functions.** Each maps to exactly one capability, and `StoreHost` decides every call with `Store::plugin_authorize`, so it is logged like any plugin call. The first set is `server` and `inventory`, both under `read_saved_state`. Function names outside the set are answered `unknown_function` without asking the host.
- **Limits.** The run is bounded at these defaults, and each has its own stop reason:
  - fuel 50 million;
  - memory 16 MiB, one memory and one table;
  - recursion depth 256;
  - 200 host calls;
  - requests 64 KiB, answers 256 KiB, results 64 KiB;
  - 50 log lines;
  - modules 4 MiB.
- **Own thread.** Every run gets its own thread with a 16 MiB stack.
- **Documents.** `docs/plugin-api/logic.md` (the contract, functions, limits and stops; a test checks it), and a link from the guide.

## Checks on 7c49d10

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 148 passed, 2 ignored (need a real SSH fixture) |
| `cargo test -p atlas-core --release --test plugin_sandbox` | pass: 6 of 6 |
| UI tests | pass: 36 of 36 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 7c49d10

All 17 suites passed in one uninterrupted run of `ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe mise run test:desktop:full`. The application does not run plugin modules yet; this run shows the change affects nothing else.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| The runtime choice and its trade-offs are recorded as a discovery note | [D060](../discovery/D060-plugin-sandbox-wasmi.md). |
| A prototype plugin calls a granted host function and is refused an ungranted one | `prototype.wat` reads `server` and `inventory`. With a real store and an installed plugin (test `the_store_host_decides_each_call_against_the_owners_grants_and_logs_it`): <ul><li>Before the plugin is enabled, both calls are answered `not_granted`.</li><li>Enabled and allowed on the server, it receives the real name and host.</li><li>With all plugins off, both calls are refused again.</li><li>Each call is in the activity log: two allowed and two refused as `read_saved_state`.</li></ul> A name outside the contract (`run_shell`) is answered `unknown_function` and the host is never asked. |
| Memory, time and call limits stop a runaway plugin without affecting the app | 16 fixture modules, each stopped with its own reason within the test's bound (test `every_runaway_and_escape_is_stopped_with_its_own_reason_quickly`): <ul><li>`out_of_fuel` for an endless loop, in `run` and in a start function;</li><li>`memory_limit` for endless growth and for 64 MiB at start;</li><li>`stack_limit` for endless recursion;</li><li>`call_limit` for a call storm and for a 2 MB request;</li><li>`contract_violation` for a pointer outside memory, an `alloc` the answer cannot fit, and no `run`;</li><li>`forbidden_import` for WASI, an imported memory and an unknown host function;</li><li>`trapped`;</li><li>`invalid_module`;</li><li>`cancelled` when permissions go away mid-run.</li></ul> A runaway on another thread did not delay a second run, which completed in well under 2 s; a release build spends the default 50 million fuel in about 0.07 s. |

Fixtures: `crates/atlas-core/tests/fixtures/plugin-sandbox/`, written by `scripts/plugin_sandbox_fixtures.py`.

## Not done, and why

- **Starting a module from the app** is not done. How a manifest declares a module, how a run is started, and how it shows in the interface belong to T049, with actions and schedules.
- **Host functions that act** are not here: running a command, files, workloads. They come with T049 and need their own owner decision, as T042 and T043 recorded.
- **Wall-clock time** is bounded by fuel, not a clock. Host calls are bounded by count and size. The first set reads only saved data, so no call can block. T049's host functions that reach servers must bring their own time limits.
- **The size wasmi adds** is not measured yet. The app does not reference the sandbox, so the linker leaves it out: the release binary grew by 16 KB.

## Notes

- **Found while testing.** With wasmi's `auto-dispatch` feature off, an endless loop overflowed the native stack and ended the whole test process, even on a thread with a 16 MiB stack. With the feature on, the same loop stops cleanly with `out_of_fuel`. D060 records that the feature must stay on.
- **A host error is data, not a trap.** A module asking for something it may not have gets an answer it can read, and continues. Only a broken contract, a limit or a cancellation stops it.
- Review: no separate review pass was run for this ticket.
