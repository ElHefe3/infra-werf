# T044 acceptance — packs for applications, badges, relationship kinds and workload kinds

Date: 2026-11-17. Feature: F21 / S12. Ticket: T044.
Revision: 368ef5c
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `368ef5c` records.

No new owner decision was needed: nothing here runs a new kind of command, changes a server or widens the tunnel boundary. Opening a pack's application is a tunnel the owner could already open by hand, and it is a change, so it follows the existing trust rule (not available to a read-only plugin; the press of Open is the confirmation for "confirm each change").

## What was built

- **Plugin API v2, extended.** Two extension points, `remote_app` and `relationship_kind`, and four contribution fields: `app` (port and path only), `badge` (which command's result it shows), `relationship` (line and colour from fixed lists) and `workload_kind` (name and state columns of a collector's table). Four error codes: `invalid_app`, `invalid_badge`, `invalid_relationship_kind`, `invalid_workload_kind`. v2 has not been released (it exists only on this branch), so it was extended rather than raised to v3; a v1 manifest is unchanged and cannot use the new points.
- **Applications.** A `remote_app` has no host field. Opening one sends only the plugin and contribution IDs; the core reads the port and path from the reviewed manifest, checks and logs each capability (`plugin_begin_app`), and calls the same `open_tunnel` as the Apps tab with the destination fixed to `localhost`. Any change to plugin permissions closes pack tunnels and their windows.
- **Badges.** A badge names a command in its own manifest that reads one value without parameters. The core binds it per server where the owner allowed both the badge and that command, and gives one of five states: healthy, unhealthy, value, stale (with the old value), unknown. A badge never runs anything; it shows the in-memory observation.
- **Relationship kinds.** Offered only between two servers the plugin is allowed on and checked again in the core when saved. The kind's title is stored with the relationship in a new additive table `edge_kind` (no schema version change; it is deleted with its relationship), so the label survives the plugin being disabled or removed; the line style then falls back to plain.
- **Workload kinds** (carried in from T043). A collector with a table can present its rows as workloads of a kind named by its title, in the Plugin data tab. A view of the latest result only: in memory, no inspection or control.
- **Interface.** The plugin page shows the application's address, the badge's source, the line style and the workload kind before enabling. Apps has a "From plugins" list; server cards show pack badges; the relationship dialog has a Kind choice; map lines take the kind's style; the activity log shows application opens.
- **Documents.** `docs/plugin-api/packs.md` gained "Adding to existing screens"; the reference and guide list the new points, fields and codes; new example `examples/homelab-map`.

## Checks on 368ef5c

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 126 passed, 2 ignored (need a real SSH fixture) |
| UI tests | pass: 32 of 32 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 368ef5c

All 14 suites passed in one uninterrupted run of `ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe mise run test:desktop:full`: desktop, ssh, health, inventory, files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps, packs, pack-map. An earlier full run stopped at the first suite (`desktop`): the inventory search assertion saw every row instead of one. That suite then passed alone and in the full rerun; it is recorded here as a flake not yet explained.

The new fourteenth suite, `pack-map` (`scripts/verify_native_pack_map.mjs`), runs the built application against the disposable Linux container over real OpenSSH, with a page served by Node on the container's own `127.0.0.1:8099`.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| A fixture pack adds an application to the Apps list that opens through the existing tunnel rules | Native `pack-map` suite: the example pack's "Status page" is listed in Apps only on the allowed server and only above read-only trust; Open started the existing loopback tunnel and the container's page answered through it at `/status`; withdrawing the tunnel permission, and turning all plugins off, each closed the tunnel within seconds. A pack whose application names another host is listed as unusable with `invalid_app`. Core tests `pack_badges_apps_and_relationship_kinds_appear_only_where_the_owner_allowed_them` and `the_map_example_is_the_documented_one_and_opens_only_on_localhost` (host, remote_host, url and bind fields, `//host/` and `http://` paths, port 0 and 70000 are all refused). |
| A badge reflects a pack observation, including stale and unknown states | Native suite: a badge shows Unknown before any run, Healthy with the container's value after the check runs, and Unknown again after a permission change forgets the observation; a badge whose command is not allowed is not drawn. Stale cannot be reached in a native run without waiting 15 minutes; it is proven by the core test `a_badge_shows_one_of_five_states_and_keeps_a_stale_value_visible` and the UI test of the badge text, which use the same freshness flag the native run reads. |
| A pack relationship kind can be chosen when documenting a relationship | Native suite: the dialog offers "Backs up to" only between two allowed servers; the saved relationship carries the kind, is drawn dashed and blue with the label "Backs up to: nightly"; a kind between servers it is not allowed on, or a contribution that is not a kind, is refused; after disabling the plugin the label stays and the line is plain. |

## Carried in from T043: what was met

| Item | Result |
| --- | --- |
| A pack can describe an additional workload kind | Done as a view: the real process table from `ps` was shown as workloads of the pack's kind in the native run. Not done: such workloads are not in the saved inventory, the Services tab or search, and have no inspection or control. That would need a pack to declare control commands, which D057 does not cover. |

## Not done, and why

- **Workload kinds beyond a view**, as above.
- **Badges cannot choose how they look.** A pack picks the observation; InfraWerf picks the five states and their look. Deliberate.
- The stop condition (a pack opening a non-loopback forward) was not approached: there is no field that could name another host, and the core always passes `localhost`.

## Notes

- **The relationship kind title is stored** (in `edge_kind`), so the database now holds a plugin's display text for each relationship documented with it. It is the owner's own map content, removed with the relationship.
- **Cancelling is still coarse.** Any plugin permission change closes every pack tunnel, not only that plugin's.
- Found while testing: the tunnel list call could drop the list of tunnels a plugin change ended before the background tick closed their windows. Fixed before this revision.
- Review: no separate review pass was run for this ticket.
