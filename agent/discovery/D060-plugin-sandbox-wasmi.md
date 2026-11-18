# D060 — Plugin logic runs in wasmi

Date: 2026-11-17. Related: S12 / F24 T048; [D052](D052-plugin-platform-direction.md) (WebAssembly as the working assumption).

## Owner decision

Asked on 2026-11-17 which WebAssembly runtime plugin logic should use, the owner chose **wasmi**.

## The comparison

Plugin logic in InfraWerf is small: decide which granted command to run, read its results, combine them, report. It runs only while the app is open. What matters is containment, limits, a small and auditable dependency, and a path to Android later. Raw speed matters little.

| | wasmi (chosen) | wasmtime | Extism on wasmtime |
| --- | --- | --- | --- |
| How it runs | Interpreter, pure Rust | Just-in-time compiler (Cranelift) | wasmtime underneath |
| Executable memory at run time | None | Yes (generated code) | Yes |
| Android later | Straightforward: no JIT, no W^X concerns | Supported, more to port and test | As wasmtime, plus its kits |
| Size added | Small: one pure-Rust interpreter crate and its parser (not measurable here yet, see below) | Large: a compiler and runtime | Largest |
| Stopping a runaway | Fuel (counted instructions), memory and table limits, recursion depth | Fuel and epoch interruption, memory limits | As wasmtime |
| Speed | Slower; a release build spends 50 million fuel in about 0.07 s here | Much faster | Much faster |
| Author experience | Plain WebAssembly from any language, with InfraWerf's small contract | The same | Ready-made kits for many languages, with Extism's own conventions |
| Audit surface | One interpreter crate and its parser | Large compiler and runtime | Larger still |

## How it is used (T048)

- **Pinned build.** `wasmi = "=2.0.0"`, with the default features off. It is built with only `std`, `validate` and `auto-dispatch`: no text format, no memory64, no SIMD, and no multi-memory in the build at all.
- **`auto-dispatch` is required.** Without it, wasmi's interpreter dispatches each instruction through calls the compiler need not turn into jumps. In an unoptimised build a long loop then overflowed the native stack and killed the whole process. Found in testing; recorded so it is not turned off again.
- **Every run gets its own thread** with a 16 MiB stack, so the caller's stack never matters.
- **Limits per run (defaults):**
  - fuel 50 million;
  - memory 16 MiB, one memory, one table of at most 10 000 elements, one instance;
  - recursion depth 256;
  - 200 host calls;
  - 64 KiB per request and 256 KiB per answer;
  - 50 log lines of 300 characters;
  - a 64 KiB result;
  - a 4 MiB module.
- **What a module may import:** only `infrawerf.call`, `infrawerf.log` and `infrawerf.result`. No WASI, and no memory or table from outside. Anything else is refused before the module runs.
- **Host functions.** Each one maps to exactly one capability, decided by `Store::plugin_authorize`, so it is logged like any plugin call. The first set is `server` and `inventory`, both under `read_saved_state`. Running commands and other actions come with T049, under their own decision.

## Size

InfraWerf does not call the sandbox from the application until T049, so the linker leaves it out of `atlas-native.exe` today: the release build grew by 16 KB, from 19 820 032 to 19 836 928 bytes. The real size cost is measured when T049 wires it in.

## Still open

- How a plugin declares its module in the manifest, how runs are started, and how they appear in the interface: T049.
- A kit for authors, for example a small Rust crate with the contract: not started.
