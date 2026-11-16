# T041 acceptance — capability model and consent screen

Date: 2026-11-16. Feature: F20 / S12. Ticket: T041.
Revision: 5fffd2a
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `5fffd2a` records. The core and commands are in `625e30c`; `5fffd2a` adds the interface, the native journey and the guide update.

## What was built

- **Finding plugins.** A plugin is a folder holding a valid `infrawerf-plugin.json` in the plugins folder beside the workspace database (`ATLAS_PLUGINS_DIR` overrides it for tests). Only the manifest is read. Unusable folders are listed with their problems. Finding a plugin allows nothing.
- **The capability model** (`crates/atlas-core/src/plugin_host.rs`). The fixed list is the registry's `Capability`. Each is a read, a change or elevation. Enabling a plugin needs a trust level: read-only, changes with confirmation, or trusted automation. Grants are per plugin, per server and per capability, and only capabilities the manifest requests can be granted.
- **Enforcement.** `Store::plugin_authorize` is the one enforcement point, in the core. It admits a call only if the plugin is enabled as reviewed, the manifest requested the capability, the owner granted it on that server and the trust level allows it. Every refusal is written to the activity history.
- **Interface** (`ui/plugins.tsx`). Settings has a Plugins section with one page per plugin: what it adds, what it asks for in plain language, the trust-level choice and per-server permissions. A server's Settings tab has a read-only "Plugins on this server" list that links to the plugin's page.
- **Storage.** Two additive tables, `plugin_state` and `plugin_grant`. The schema version stays 3, so the previous release still opens the file.

## Decisions made in this ticket

| Decision | Reason |
| --- | --- |
| Every capability is granted per server, including `read_saved_state` and `edit_map` | They concern one server's saved data or map entry. One rule is easier for the owner to reason about than two scopes. |
| Trust level is per plugin; the per-server choice is which capabilities are allowed | D055 has both "per plugin" and "per server" wording. The level says how far the plugin may act at all; the server grants say where. |
| Disabling forgets the trust level but keeps server permissions | Enabling again must be a fresh, explicit choice. The kept permissions are shown on the page and do nothing while the plugin is disabled. |
| A manifest that changed after review switches the plugin off | The owner agreed to a specific manifest. Its hash is stored at enable; T046 will refine this to re-ask only when capabilities grow. |
| Sudo cannot be granted | D055 allows only a named exception for one plugin, one server and one action. A v1 manifest cannot name an action, so there is nothing to make an exception for. `elevate` is always refused; granting exceptions belongs to T042 once F21 defines actions. |
| Plugin contributions are not added to the extension registry yet | API v1 has no plugin content, so there would be nothing to draw or run. An allowed plugin therefore adds nothing visible yet. |
| No schema version bump | New tables only, created if absent on open. The owner's installed release keeps working on the same file. |

## Checks on 5fffd2a

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 111 passed, 2 ignored (real-SSH fixture tests) |
| UI tests | pass: 28/28 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 5fffd2a

All twelve pass in a single run: desktop, ssh, health, inventory, files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps. The desktop suite includes the plugin journey and reports: "plugins: found with no trust and no grants; consent lists each capability in plain language; enabling needs an explicit trust level; grant, use, withdraw and disable are enforced in the core per server and logged; the server Settings tab shows a read-only list that links to the plugin page".

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| A plugin call outside its granted capabilities is refused in the core and logged | `tests/plugin_host.rs` attempts all twelve capabilities in every state: found but not enabled, enabled with nothing granted, granted on another server, each trust level, withdrawn, disabled, manifest changed, server removed, unknown plugin and unknown server. Refusals are counted in the activity history. The native journey makes nine refused calls through the real IPC command and finds all nine in the history. |
| The consent screen lists each requested capability in plain language, including the exact commands a declarative pack will run | The journey reads the three capability sentences and the "Used by" titles on the plugin page. The sentences come from the core (`capability_summary`). **The commands part cannot be shown yet:** declarative packs do not exist until F21. T043 must add each pack's exact commands to this page; that is recorded as a carry-over, not as done. |
| One configuration page per plugin shows every permission it has and is the only place they are changed | The page shows the trust level and every server's permissions. The server Settings list is asserted to contain no checkbox or radio and to link to the page. The only commands that change permissions are `plugin_enable`, `plugin_disable` and `plugin_set_grant`, called only from that page. |
| Grants can be given and withdrawn per server, from that page | The journey ticks one capability on one server, sees it admitted there and refused on another server, withdraws all and sees it refused, then allows all. |
| Enabling a plugin requires an explicit trust-level choice; no level is preselected and none is implied | The journey asserts three radios with none checked and a disabled Enable button, before the first enable and again after disabling. In the core there is no way to enable without a `TrustLevel`, and a found plugin has `trust: null`. |
| No capability gives access to keys, passwords or an unrestricted shell | The manifest cannot request `interactive_terminal` (T039), and the core test finds it refused in every state. `elevate` is refused at every trust level and cannot be granted (`InvalidInput`). `ssh_session` is brokered: nothing in this ticket hands a plugin a key or a session. |

## Carry-overs

- **T043:** show a pack's exact commands on the plugin page (second criterion).
- **T042:** elevation exceptions; a separate plugin activity log. Refusals currently go into the general bounded history, so a plugin that is refused very often could push older events out.
- **T046:** re-ask only when capabilities grow, instead of on any manifest change.

## Notes

- `docs/plugin-api/guide.md` now describes the real review page instead of saying it does not exist (the T040 carry-over).
- The Settings screen is drawn inside the map panel under the page heading, so a long plugin page scrolls in a short area on small windows. That predates this ticket (T052) and is not changed here.
- Review: no separate review pass was run for this ticket.
