# T037 progress — registries for tabs, badges, collectors and operations

Date: 2026-11-12. Feature: F18 / S12. Ticket: T037 (in progress: implemented and unit-proven; native proof is partial, so this is not an acceptance record).
Revision: working tree on top of `ffccc7a` on branch `feature/s12-extensibility`; committed with this record. Windows 11 x64; release binary `target/release/atlas-native.exe` from `mise run build:desktop`. Tool pins unchanged from [CHECKPOINT.md](CHECKPOINT.md).

## What was built

- `atlas-core::extensions`: `ExtensionPoint`, `Capability`, `Contribution`, `ExtensionRegistry` and `builtin()`. The built-in registry holds 29 contributions: 8 workspace tabs, 3 server-card badges, 3 settings sections, 1 collector, 2 health checks and 12 operations. Registration rejects malformed or duplicate IDs, unbounded titles, empty or repeated capability lists and capabilities the extension point does not permit.
- Enforcement in the core: `OperationBroker::prepare` authorises every `Inspection` against the registry before any session check. The workspace authorises health checks (`configure_health`), the baseline inventory collector (`refresh_inventory`) and remote-application tunnels (`open_tunnel`) the same way. An absent contribution returns `not_found`; an undeclared capability returns `denied`.
- Contracts: the four types are exported to `ui/contracts.gen.ts`; a new main-window command `extensions_state` returns the catalogue.
- Interface: `ui/extensions-model.ts` binds catalogue entries to renderers and rejects a renderer with no registration or a registration with no renderer. `ui/extensions.tsx` holds the built-in renderers. `ui/main.tsx` draws workspace tabs, server-card badges and Settings sections from the bound catalogue; the hard-coded tab list is gone. Tab titles, DOM classes and order are unchanged.
- No security boundary changed. Nothing is loaded from outside the binary; every contribution is owned by `builtin`.

## Checks

| Check | Result |
| --- | --- |
| `mise run check` before any change (baseline) | pass |
| `mise run check` after the change | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 91 passed, 2 ignored (real-SSH fixture tests) |
| `mise run test:feature -- F18` | pass: 5 registry tests, 8 operations tests, 3 shell tests, contracts, tsc, 20 UI tests, tracker |
| New tests | 5 in `extensions::tests`, 1 in `operations::tests` (broker refuses unregistered and undeclared operations), 4 in `ui/extensions-model.test.ts` |
| `mise run build:desktop` | pass: release binary and NSIS bundle |
| `check_tracker.py --self-test` | pass |

## Native suites: new build compared with the installed 0.1.5 baseline

The baseline is the installed `atlas-native.exe`, the release build of `c0f8299` (the parent of the S12 work). Each suite ran alone on its own synthetic database and loopback fixture.

| Suite | T037 build | 0.1.5 baseline |
| --- | --- | --- |
| desktop | pass | not rerun |
| health | pass | not rerun |
| monitoring | pass | not rerun |
| apps | pass | not rerun |
| ssh | fail | fail, same line |
| inventory | fail | fail, same line |
| files | fail | fail, same line |
| file-changes | fail | fail, same line |
| terminal | fail | fail, same line |
| elevation | fail | fail, same line |
| workloads | fail | fail, same line |
| workload-control | fail | fail, same line |

All eight failures happen at the suite's first `add_server` call, before any T037 code path, and are identical on the baseline. See [D054](../discovery/D054-native-suites-conflict-with-alias-discovery.md). These runs show no native regression from T037, but eight suites cannot currently prove anything.

## Acceptance mapping

| Criterion | State |
| --- | --- |
| Tabs and badges rendered from a registry entry | Met: `ui/main.tsx` renders only bound contributions; the desktop, health, monitoring and apps suites pass on that interface. |
| Collectors and health checks enumerated from a core registry | Met: `builtin()` plus authorisation in `configure_health` and `refresh_inventory`; the health and monitoring suites pass through those paths. |
| All existing native suites pass unchanged | **Not met.** Four pass; eight fail identically on the baseline for a reason that predates T037. |
| Each registry entry declares its capabilities | Met: registration rejects an empty list; `builtin_registry_covers_every_extension_point` asserts it for all 29. |

## What the native runs did and did not cover

Covered on the T037 build: map and workspace tabs (desktop), the Health tab and health-check authorisation (health), inventory refresh through the collector check and SSH verification (monitoring), and an authenticated session, the brokered `ApplicationDiscovery` operation and tunnel authorisation (apps).

Not covered natively: the Files, Terminal and Settings (SSH, elevation) tabs and workload control as rendered through the registry, and the brokered SFTP, terminal, elevation and workload operations. Their authorisation is covered by unit tests only.

## Remaining before closure

1. Decide how to repair the eight suites (D054), then rerun them on this build.
2. Independent review of the registry and the broker check.
3. Record acceptance evidence with the tested revision.
