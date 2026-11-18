# T038 acceptance — remote applications as a built-in extension

Date: 2026-11-15. Feature: F18 / S12. Ticket: T038.
Revision: dd69f9c
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `dd69f9c` records. The feature code is in `2b6a22c`; `0509515` and `dd69f9c` add two fixes found by this ticket's full runs (see Notes).

## What was built

- An **extension** is now the unit that owns contributions and is enabled or disabled as a whole. The catalogue lists every extension (`ExtensionInfo`: id, title, enabled, optional) and the contributions of the enabled ones.
- **Remote applications is `builtin.remote-apps`**, optional. It owns the Apps tab (`builtin.tab.apps`), application discovery (`builtin.operation.application-discovery`) and the loopback tunnel (`builtin.operation.loopback-tunnel`). Everything else is owned by `builtin`, the core, which cannot be disabled.
- **Disabling it:** start InfraWerf with `ATLAS_DISABLED_EXTENSIONS=builtin.remote-apps`. Its contributions are not registered, so the tab is not drawn and both operations are refused where they were already authorised. The six tunnel commands (`tunnel_open`, `tunnel_discover`, `tunnel_close`, `tunnel_close_all`, `tunnel_list`, `tunnel_focus`) refuse with `not_found`.
- No remote-application behaviour changed, and the loopback-only boundary (D051) is untouched.

## Gaps found in the extension contracts, and how each was fixed

| Gap | Fix in the contract |
| --- | --- |
| There was no unit to disable: every contribution had the same owner, `builtin`. | `ExtensionInfo`, `ExtensionRegistry::declare`, and an `extensions` list in `ExtensionCatalogue`. A contribution's `owner` is its extension. |
| Nothing could be left out at start-up. | `builtin_with(disabled)`; a non-optional extension cannot be declared disabled. |
| Commands with no capability to check (list, close, close all, focus) could not be turned off through `authorize`. | `ExtensionRegistry::require(extension)`, called first by every command of the extension. |
| The interface treated a renderer without a registration as an error, so a disabled built-in extension would have stopped the interface from loading. | `forEnabled` leaves out renderers whose extension is listed as disabled. Binding stays strict for everything else, including an enabled extension with a missing contribution. |

No private back door into the core was needed.

## Checks on dd69f9c

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 95 passed, 2 ignored (real-SSH fixture tests) |
| UI tests | pass: 28/28 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on dd69f9c

All twelve pass in a single run: desktop, ssh, health, inventory, files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps. `scripts/verify_native_apps.mjs` is unchanged.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Disabling the built-in extension removes the Apps tab and its commands with no other effect | The desktop suite restarts the real application with the extension disabled. The catalogue differs by exactly the three contributions owned by `builtin.remote-apps`; the workspace shows the same tabs minus Apps; all six tunnel commands return `not_found`; servers, relationships and history are identical; no console errors. A normal restart restores the full catalogue. The registry unit test proves the same set difference in the core. |
| The remote-applications native suite passes unchanged | `verify_native_apps.mjs` has no diff on this branch since T035 and passes in the full run. |
| Gaps found in the extension contracts are fixed in the contracts and listed | The table above. |

## Notes

- The switch is an environment variable read at start-up, not an option in Settings. Owner-facing enable and disable belongs to F20/F22, which will store the choice; this ticket proves the seam.
- The first full run for this ticket failed in the T053 menu journey: a floating menu stopped following the pointer after the first move while the machine was under load. Drags relied on pointer capture; `0509515` tracks them on the window instead. The desktop suite then passed three times in a row under load.
- The second full run failed in the files suite, at a step this ticket does not touch: after a host edit, the Files view lost its "Protected file data has been cleared" notice when the map refresh remounted it. Alone, the suite failed about two runs in three, so this race predates the ticket and earlier passes were luck. `dd69f9c` carries the notice across the remount; the files suite then passed six times in a row and the third full run passed.
- The commit message of `2b6a22c` says all twelve suites passed. That was written before the result was read and was not true until `dd69f9c`.
- Review: no separate review pass was run for this ticket.
