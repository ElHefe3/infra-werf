# Service summary — example plugin

A complete plugin [job](../../docs/plugin-api/logic.md#declaring-a-job): a WebAssembly module that reads a server's workloads through InfraWerf and returns them as its result. It changes nothing, so it asks only for `read_saved_state`.

- `infrawerf-plugin.json` declares the job.
- `job.wat` is the module's source, in the WebAssembly text format.
- `job.wasm` is the module InfraWerf runs. InfraWerf's tests rebuild it from `job.wat` and fail if the two differ.

Any language that compiles to WebAssembly without WASI works, as long as the module exports `memory`, `alloc` and `run` and imports only `infrawerf.call`, `infrawerf.log` and `infrawerf.result` ([the contract](../../docs/plugin-api/logic.md)).

Check it:

```
mise run exec -- cargo build -p atlas-core --bin infrawerf-plugin
target\debug\infrawerf-plugin.exe check examples\service-summary
```

Once installed and enabled, allow it to read saved state on a server; the job appears in that server's Plugin data tab with **Run now**, and its schedule is set on the plugin's page.
