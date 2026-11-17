# T046 acceptance — updates that re-ask when capabilities grow

Date: 2026-11-17. Feature: F22 / S12. Ticket: T046.
Revision: 379bb0e
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64; release binary `target/release/atlas-native.exe` from `pnpm tauri build --no-bundle -- --locked`, built from the tree that `379bb0e` records.

## What was built

- **Update on a plugin's page.** The new version comes from a folder, an archive or an `https://` address, with the same checks, optional expected SHA-256 and optional public key as an install (T045). It must have the same plugin ID.
- **Check, then apply.** Checking reads and validates the update and shows what changes; nothing is replaced. The checked files are kept in memory for 15 minutes under a token, and applying uses exactly those files: nothing is fetched or read again. Applying is refused if the installed manifest changed in between, or if the token was used or expired.
- **What the owner sees:** the versions and a downgrade warning, capabilities asked for in addition and no longer, every new, removed or changed contribution with its new command written out, the previous and new signer key, and the new files hash.
- **The rule.** An update keeps the owner's review and keeps working only if it asks for nothing new: no added capability, no new contribution, and no changed command, application, point or capability use. Otherwise its recorded review no longer matches, so it is switched off as "changed since you reviewed it" until the owner enables it again. Permissions are never added by an update; permissions for capabilities it no longer requests are deleted. Replacing a plugin whose files had changed always needs a new review.
- **Downgrades** are shown and apply only when the owner ticks a confirmation; the core refuses one without it.
- **Previous version kept until the new one is in place.** The update is written to a staging folder and checked, the installed folder is renamed aside, the new one renamed in, then the records are saved in one transaction. If moving it in or saving fails, the previous folder is put back.
- **Activity log.** Every applied update ("1.0.0 to 1.0.1", ", downgrade", ", needs your review") and every refused one, with its reason.
- **Documents.** "Updating" in `docs/plugin-api/install.md` with the rule and four new error codes.

## Checks on 379bb0e

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 140 passed, 2 ignored (need a real SSH fixture) |
| UI tests | pass: 34 of 34 |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

## Native suites on 379bb0e

**Not one uninterrupted run; closed on the owner's instruction (2026-11-17: "Close it with a note").** Every one of the 16 suites passed on this release binary, but no single run of all 16 did. Four full runs each stopped on a different suite unrelated to plugins: `desktop` (main menu detach), `files`, `file-changes` (both at file-browsing steps), and `apps` (the final Quit click, after all its checks passed). Each of those passed when run again on its own or in a later run: `files` 3 of 3 alone, `file-changes` through `pack-map` in one run from `-StartAt file-changes`, `plugin-install` and `plugin-update` together, `desktop` alone. The `files` flake is not new: running `inventory` then `files` twice on each build failed once on the T045 binary and once on this one, at the same step. Two real test breaks caused by this ticket were found and fixed: the desktop suite counted every radio on the plugin page (the Update form adds three), and the install suite clicked a label that was renamed.

The new suite `plugin-update` (`scripts/verify_native_plugin_update.mjs`) drives the Update panel against the loopback HTTPS fixture, which serves each fixture version as a zip.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| An update requesting a new capability stays inactive until reviewed | Fixture `added-capability` (adds `loopback_tunnel` and an application): the panel lists it under "Asks for more"; after applying, the plugin refuses everything as `changed_since_review` until enabled again, and even at trusted automation `loopback_tunnel` stays `not_granted`. The same holds for `changed-command` (same capabilities, `uname -r` to `uname -a`) and `new-contribution` (a new command). Core test `an_update_asking_for_anything_new_stays_off_until_reviewed_and_grants_nothing`; native suite steps 2 and 3. |
| An update with the same or fewer capabilities applies and is logged | Fixtures `same` (a title changes) and `fewer` (drops `network_probe` and a contribution): applied without review, still enabled with the same trust, the dropped capability's permission deleted, logged as "1.0.0 to 1.0.1". Core test `an_update_asking_for_nothing_new_applies_keeps_the_review_and_is_logged`; native suite steps 1 and 6. |
| A failed update leaves the previous version working | Fixtures `invalid` (a shell as program) and `different-plugin` refused while checking; an update whose installed manifest changed after checking refused with `changed_since_preview`; a token used twice refused with `preview_expired`; a downgrade without confirmation refused. After each the installed files were byte-for-byte the same and the plugin kept working (native suite step 5). Core test `a_failed_update_leaves_the_previous_version_working` also swaps the new files in with `replace` and puts the old ones back with `restore`, as happens when the records cannot be saved. |

Fixtures: `crates/atlas-core/tests/fixtures/plugin-updates/<case>/v1` and `v2` for `same`, `fewer`, `added-capability`, `changed-command`, `new-contribution`, `downgrade`, `different-plugin` and `invalid`.

## Carried in from T045: what was met

| Item | Result |
| --- | --- |
| An update goes through the same checks as an install and keeps the previous folder until the new one validates | Done. |
| A change of signer is shown before applying | Done: "Signed by the same key as before", "by a different key", or "not checked" with the previous key ID. It is shown, not enforced: the owner decides (D058 trusts no key by default). |

## Not done, and why

- **Requiring the same signing key** for updates. Shown, not enforced; enforcing it would need its own owner decision.
- **Rolling back after an update applied.** The previous version is deleted once the update's records are saved; going back means installing the old version as a (confirmed) downgrade.

## Notes

- **The rule is stricter than the ticket's wording.** The ticket asks for a new review when capabilities grow. A changed command with the same capabilities would run something the owner never read, which D057 does not allow, so any new or changed command, application or access also needs a review. A downgrade that only removes things keeps working, by the same rule.
- **"Changed since checking" compares the manifest's hash**, not every file; the files are replaced from the checked copy either way.
- **Applying stops pack commands in flight and closes pack tunnels**, like any plugin change.
- Review: no separate review pass was run for this ticket.
