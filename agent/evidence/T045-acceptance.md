# T045 acceptance — install, remove and verify plugins

Date: 2026-11-17. Feature: F22 / S12. Ticket: T045.
Revision: 6777941
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `6777941` records.

Owner decision first: HTTPS built in, minisign signatures with no key trusted by default, and both zip and `.tar.gz` ([D058](../discovery/D058-plugin-install-sources-and-signatures.md), 2026-11-17).

## What was built

- **Sources.** Settings → Plugins → Install a plugin: a folder, a `.zip` or `.tar.gz` file (recognised by content), or an `https://` address. Optional expected SHA-256 and optional minisign public key. Folder and file are chosen with the native picker.
- **Checks before anything is written** (`crates/atlas-core/src/plugin_install.rs`). Archive or download at most 5 MiB; files 20 MiB in all, 5 MiB each, 256 files, 200-character paths, 8 folders deep, enforced on what is read, not on headers, including a bound on the decompressed tar stream. Paths must be relative plain names safe on Windows (no `..`, `.`, absolute paths, drives, device names, trailing dots or spaces, `< > : " | ? *`, control characters, or names equal ignoring case). Only plain files and folders: links, hard links, devices and reparse points are refused from archives and folders. The manifest must be at the top or in one single top folder, and valid.
- **Publish.** Files are written to `plugins/.staging-<id>`, read back and compared with the checked hash, then renamed into `plugins/<plugin id>` in one step. Any failure removes the staging folder. The install record (source, files SHA-256, archive SHA-256, verified key ID, time) is written in the same transaction as the activity entry; if it cannot be written the folder is removed again.
- **Disabled until reviewed.** Installing grants nothing, and installing a plugin again clears any trust and permissions left under its ID.
- **Start-up check.** At every start-up InfraWerf removes interrupted working folders and recomputes each installed plugin's files hash. A changed, added or missing file marks it `files_changed`: its status says so, it cannot be enabled, and every capability is refused with `files_changed` until it is removed and installed again. The hash, the archive hash, the signer and the last check are shown on its page.
- **Removal.** Remove (with confirmation) on a plugin's page, or on an unusable folder, renames the folder out of the way, deletes trust, permissions and the install record, logs the removal, then deletes the files. Pack commands in flight are cancelled and pack tunnels closed. The activity log keeps the plugin's history.
- **HTTPS.** `ureq` with rustls and Mozilla's roots, `https` only (a redirect to `http` fails), five redirects at most, 30 seconds, 5 MiB, no credentials in the address.
- **Documents.** `docs/plugin-api/install.md` (new; a test checks it lists every code and limit), the guide's sharing section, `scripts/plugin_archive_fixtures.py` to regenerate the fixtures.

## Checks on 6777941

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 135 passed, 2 ignored (need a real SSH fixture) |
| UI tests | pass: 33 of 33 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 6777941

All 15 suites passed in one uninterrupted run of `ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe mise run test:desktop:full`: desktop, ssh, health, inventory, files, file-changes, terminal, elevation, workloads, workload-control, monitoring, apps, packs, pack-map, plugin-install. An earlier full run stopped at the fourth suite (`inventory`) because its Linux container did not start within 30 seconds, before any InfraWerf code ran; the rerun passed.

The new suite `plugin-install` (`scripts/verify_native_plugin_install.mjs`) drives the built application with a loopback HTTPS server whose throwaway certificate authority is trusted only by that InfraWerf process (`ATLAS_PLUGIN_TEST_CA`).

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| A plugin installs from each of the three sources and appears disabled until reviewed | Native suite: through the form from `https://127.0.0.1:<port>/good.zip` with the expected SHA-256 and the author's public key (page shows the address, both hashes, key ID `BAEDC8198EBE6C24`, "Files match"); from a `.tar.gz` file; from the `examples/homelab-map` folder. Each is disabled with no permissions. Core test `a_plugin_installs_from_a_folder_and_from_each_archive_kind_and_is_disabled`. |
| An archive with path traversal, oversize content or a hash mismatch is refused with nothing left behind | 16 fixture archives, each refused with the code in its name and the plugins folder empty afterwards (core test `each_bad_archive_is_refused_with_its_own_code_and_leaves_nothing_behind`, and again through the built application): `..`, backslash `..`, absolute path, drive letter, names equal ignoring case; tar symlink and hard link, zip symlink; 30 MiB zip and tar.gz bombs of about 30 KiB; 300 files; no manifest; two top folders; not an archive; reserved plugin ID; a signed archive changed after signing. Also refused natively: a redirect to `http`, a 6 MiB download, an `http://` address, a missing address, a wrong expected hash. Core test `hashes_and_signatures_are_checked_when_the_owner_asks` covers wrong key, malformed key, missing signature and a key given for a folder. |
| Removal clears files, grants and schedules | Native suite and core test `removal_clears_files_trust_permissions_and_the_record_but_keeps_history`: folder, trust, permissions on every server and install record gone; installing again gives a disabled plugin with no permissions; the activity log keeps the history. **Schedules do not exist yet** (T049); removal also cancels pack commands in flight and closes pack tunnels, which is the work that does exist. |
| The installed hash is shown and re-checked at start-up | Native suite: restarted unchanged it stays verified and allowed; after `README.md` was changed while InfraWerf was closed, the next start-up showed "Files changed since it was installed", switched it off, removed the trust choice and refused with `files_changed`. Core test `changed_files_are_found_at_start_up_and_refuse_everything` also covers an added file, putting it back, and leftover working folders. |

## Not done, and why

- **Schedules** are not removed because none exist; T049 must remove a plugin's schedules on removal. Added to T049's carried-in list.
- **Updates.** Installing a plugin whose ID is already installed is refused with `already_installed`; updates are T046.
- **Hand-placed folders** have no recorded hash and are not checked at start-up; their manifest is still checked for changes as before. The plugin page says so.

## Notes

- **Signatures were tested against a Python implementation of minisign's format** (`scripts/plugin_archive_fixtures.py`), not files from the minisign tool itself. The format is small and documented, and `minisign-verify` is the reference Rust verifier, but no interoperability run with the real tool was done.
- **The start-up check reads every installed plugin's files** (at most 20 MiB each). With many large plugins start-up gets slower; no measurement was made.
- **First outbound internet access.** Only when the owner presses Install with an address. Proxy settings from the environment are honoured by the HTTP library's defaults.
- Found while testing: removing a plugin from its page cleared the confirmation message; and a relaunch could be driven before the workspace loaded. Both fixed before this revision.
- Review: no separate review pass was run for this ticket.
