# T037 acceptance — registries for tabs, badges, collectors and operations

Date: 2026-11-13. Feature: F18 / S12. Ticket: T037.
Revision: 33474bc
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`. Tool pins unchanged from [CHECKPOINT.md](CHECKPOINT.md).

## What was built

- `atlas-core::extensions`: `ExtensionPoint`, `Capability`, `Contribution`, `ExtensionRegistry` and `builtin()`. The built-in registry holds 31 contributions: 8 workspace tabs, 3 server-card badges, 3 settings sections, 1 collector, 2 health checks and 14 operations (11 inspections, the SSH connection check, SFTP open and the remote-application tunnel). Registration rejects malformed or duplicate IDs, unbounded titles, empty or repeated capability lists and capabilities the extension point does not permit.
- Enforcement in the core: `OperationBroker::prepare` and both session bootstraps authorise against the registry before any session check. The workspace authorises health checks, the baseline inventory collector and tunnels the same way. An absent contribution returns `not_found`; an undeclared capability returns `denied`. The registry is held in the broker's shared state, so a replacement reaches every clone.
- Contracts: the four types are exported to `ui/contracts.gen.ts`; the main-window command `extensions_state` returns the catalogue.
- Interface: `ui/extensions-model.ts` binds catalogue entries to renderers and rejects a renderer with no registration or a registration with no renderer. `ui/main.tsx` draws workspace tabs (selected by contribution ID), server-card badges and Settings sections from the bound catalogue. Titles, DOM classes and order are unchanged. The map is shown only once the catalogue is bound.
- No security boundary changed. Nothing is loaded from outside the binary; every contribution is owned by `builtin`.

## Checks on 33474bc

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 94 passed, 2 ignored (real-SSH fixture tests) |
| `mise run test:feature -- F18` | pass |
| UI tests (`extensions-model`, `client`, `map-model`, `live-model`, `remote-apps-model`) | pass: 20/20 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 33474bc

Each suite ran alone on its own synthetic database and loopback fixture.

| Suite | Result |
| --- | --- |
| desktop | pass |
| ssh | pass |
| health | pass |
| inventory | pass (second run) |
| files | pass (second run) |
| file-changes | pass (second run) |
| terminal | pass (second run) |
| elevation | pass (second run) |
| workloads | pass (second run) |
| workload-control | pass (second run) |
| monitoring | pass |
| apps | pass |

Seven suites needed a second run for an environment reason: the podman machine that hosts the Linux fixture was stopped. Five of them reported "Linux fixture setup failed" before reaching the application; files and file-changes failed in the same run and passed unchanged once the machine was started. No code changed between the two runs.

One flake was seen on the preceding build: file-changes failed once on a folder-navigation timing assertion and passed on two immediate reruns of the same binary.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Tabs and badges rendered from a registry entry | `ui/main.tsx` renders only bound contributions; every native suite drives that interface. |
| Collectors and health checks enumerated from a core registry | `builtin()` plus authorisation in `configure_health` and `refresh_inventory`; the health, inventory and monitoring suites pass through those paths. |
| All existing native suites pass unchanged | All twelve pass. One disclosed exception to "unchanged": the inventory suite's stale activity-text assertion was updated for a reason unrelated to T037 ([D054](../discovery/D054-native-suites-conflict-with-alias-discovery.md)). |
| Each registry entry declares its capabilities | Registration rejects an empty list; `builtin_registry_covers_every_extension_point` asserts it for all 31. |

## Work done alongside T037

- [D054](../discovery/D054-native-suites-conflict-with-alias-discovery.md): eight native suites failed at `add_server` on the first T037 build and identically on the installed 0.1.5 release. With the owner's choice, `add_server` now adopts, and `update_server` replaces, a host held only by a discovered candidate (`28c9efc`, refined in `33474bc`).

## Review

A structured review pass over `1b4fd71` and `28c9efc` was run on 2026-11-13 at the owner's request, by the same agent that wrote the code, not by a separate reviewer. It reported ten findings. Nine are fixed in `33474bc`:

1. A host edit could silently delete a candidate's relationships: a candidate with relationships is no longer replaced.
2. A replaced candidate's sessions and health checks were not ended: the shell now ends them.
3. An adopted server stayed hidden until the next poll: map and monitoring now refresh together.
4. The registry was not shared across broker clones: it is now in shared state, with a test.
5. The session bootstraps bypassed the registry: both are registered and authorised.
6. Tabs were selected by title: they are selected by contribution ID.
7. A failed catalogue load left a half-working map: the map now waits for the catalogue.
8. The tunnel contribution ID was a repeated literal without a test: now a constant with a test.
9. Server deletion logic was duplicated: one helper.

One is left open by design: `authorize` proves that the operation about to run is registered and declared its capability, but does not know which tab or plugin asked. Caller identity and owner grants are F20 (T041). The limit is stated in the module documentation.

The review also caused one regression, caught by the monitoring suite and fixed before `33474bc`: delaying the map until the catalogue loaded also delayed the first monitoring load, and the monitoring form saved default settings in that window. Monitoring now loads with the catalogue and the form is disabled until it has.
