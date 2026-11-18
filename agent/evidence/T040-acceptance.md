# T040 acceptance — plugin author guide and example plugin

Date: 2026-11-15. Feature: F19 / S12. Ticket: T040.
Revision: 8aff7ed
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64. The guide and example are in `2c6671c`; `8aff7ed` adds the changes the walkthrough led to.

## What was built

- `docs/plugin-api/guide.md`: three steps from an empty folder to a valid manifest, one of them deliberately refused to show how to read an error; then extension points, capabilities, versions and a checklist.
- `examples/backup-overview`: the finished manifest.
- Tests in `crates/atlas-core/tests/plugin_manifest.rs` keep them honest (see the mapping below).

## Checks on 8aff7ed

| Check | Result |
| --- | --- |
| `mise run check` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 104 passed, 2 ignored (real-SSH fixture tests) |
| `check_tracker.py --self-test` | pass |

Native suites were not run: no application behaviour changed.

## Cold-read walkthrough

Reader: the owner, on 2026-11-15, working from the guide as shown in the conversation (revision `2c6671c`) in their own PowerShell terminal. The owner had not read the guide, the validator or the example before. They pasted the terminal session; the notes below are read from it plus their one remark.

| Step | What the reader did | Result |
| --- | --- | --- |
| 1 | Saved the first manifest outside the repository and ran the check command on the file. | `org.example.backup-overview 0.1.0 is valid for plugin API v1 (1 contribution, 1 capability requested).` Matches the guide. |
| 2 | Replaced the contents with the second manifest and ran the check, twice. | Refused with the two `capability_not_requested` lines exactly as the guide quotes them. |
| 3 | Replaced the contents with the final manifest and ran the check. | `org.example.backup-overview 1.0.0 is valid for plugin API v1 (2 contributions, 3 capabilities requested).` Matches the guide. |

The reader built the example from the guide alone and every output matched. Criterion met.

What the session showed, and what changed in `8aff7ed`:

| Observation | Change |
| --- | --- |
| Every run printed about 45 lines of `ts-rs failed to parse this attribute` warnings before the one line that mattered. | The warnings are silenced at the source (`ts-rs` feature `no-serde-warnings`); they were never actionable. This also quiets every other build. |
| The expected refusal in step 2 ended with `error: process didn't exit successfully` and `[exec] ERROR task failed`, which reads like a second failure. The reader ran that step twice. | The guide now has the reader build the tool once and run `target\debug\infrawerf-plugin.exe` directly, which prints only the tool's output. The wrapper form is still documented, with a sentence saying what those two lines mean. Step 2 now says the refusal is expected. |
| The reader first saved the file as `infrawerf-pugin.json` (their remark: "I fixed the spelling mistake while testing"). The check passed anyway, because a path to a file is accepted whatever its name. | The guide now says the path may be the file or its folder, that a file's name is not checked, and that it must be `infrawerf-plugin.json` before sharing. |
| The reader checked a file, and kept it outside the repository; the guide only said `<folder>`. | The guide says both forms work and that the plugin folder can be anywhere. |
| Steps 2 and 3 did not say to replace the file's contents or to run the check again. | Both now say so. |

Limits of this record: it is one reader who knows the project well, so it does not show how a stranger to InfraWerf would get on with `mise` and the repository set-up. The reader did not report where they hesitated beyond the remark above.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| The guide documents every manifest field and extension point in API v1 | Test `the_guide_and_reference_document_everything_in_api_v1` reads the field tables and the generated schema and fails if the guide or the reference omits any field, extension point or capability, or the reference omits any error code. |
| The example plugin validates and loads in an automated test | Test `the_example_plugin_validates_and_loads` validates it and registers its contributions in the same registry as the built-in ones. Test `the_guide_builds_the_example_and_its_steps_behave_as_written` runs every manifest in the guide and requires the last one to equal the example. |
| A person who has not seen the code can build the example from the guide alone (recorded walkthrough) | The walkthrough above. |

## Notes

- **"The review a user sees."** The ticket's included work asks the guide to cover it. The in-app review is not built (F20), and the stop condition forbids describing behaviour that is not implemented. The guide says the screen does not exist yet, shows today's check output and lists the manifest fields the review will be based on. That section must be rewritten when T041 lands; it is not an acceptance criterion.
- Review: no separate review pass was run for this ticket.
