# T037 progress — registries for tabs, badges, collectors and operations

Date: 2026-11-12. Feature: F18 / S12. Ticket: T037 (in progress: all four criteria demonstrated; independent review pending, so this is not an acceptance record).
Tested revision: `28c9efc` on branch `feature/s12-extensibility` (T037 is `1b4fd71`; `28c9efc` adds the D054 fix). Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`. Tool pins unchanged from [CHECKPOINT.md](CHECKPOINT.md).

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

## Native suites

Each suite ran alone on its own synthetic database and loopback fixture, against the release build of `28c9efc`.

| Suite | Result |
| --- | --- |
| desktop | pass |
| ssh | pass |
| health | pass |
| inventory | pass |
| files | pass |
| file-changes | pass |
| terminal | pass |
| elevation | pass |
| workloads | pass |
| workload-control | pass |
| monitoring | pass |
| apps | pass |

On the same revision: `mise run check` passes and `cargo test --workspace --locked -- --test-threads=1` gives 94 passed, 2 ignored.

### How the suites got here

On the T037 commit alone (`1b4fd71`), eight suites failed at their first `add_server`, identically on the installed 0.1.5 release (`c0f8299`): ssh, inventory, files, file-changes, terminal, elevation, workloads and workload-control. The cause predates S12; see [D054](../discovery/D054-native-suites-conflict-with-alias-discovery.md). The owner chose to fix the product. `28c9efc` makes `add_server` adopt, and `update_server` replace, a host held only by a discovered candidate. One suite assertion changed: the inventory suite now also accepts the first-collection activity text introduced by `dc66c0f`. No other suite was edited.

## Acceptance mapping

| Criterion | State |
| --- | --- |
| Tabs and badges rendered from a registry entry | Met: `ui/main.tsx` renders only bound contributions; every native suite drives that interface. |
| Collectors and health checks enumerated from a core registry | Met: `builtin()` plus authorisation in `configure_health` and `refresh_inventory`; the health, inventory and monitoring suites pass through those paths. |
| All existing native suites pass unchanged | Met with one disclosed exception: all twelve pass; the inventory suite's stale activity-text assertion was updated, for a reason unrelated to T037. |
| Each registry entry declares its capabilities | Met: registration rejects an empty list; `builtin_registry_covers_every_extension_point` asserts it for all 29. |

The brokered SFTP, terminal, elevation and workload operations, and the Files, Terminal and Settings tabs, are now exercised natively through the registry by their suites.

## Remaining before closure

1. Independent review of the registry and the broker check, or an owner decision to close without one.
2. Then record `Result: pass` here and set the ticket to `done`.
