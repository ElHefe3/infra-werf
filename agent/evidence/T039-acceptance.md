# T039 acceptance — plugin manifest and API version

Date: 2026-11-15. Feature: F19 / S12. Ticket: T039.
Revision: 8c908f4
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64.

## What was built

- **Format:** one JSON file per plugin, `infrawerf-plugin.json`, UTF-8, 64 KiB at most.
- **Types and validator:** `crates/atlas-core/src/plugin.rs`. `validate_manifest` returns the manifest or every problem found, each with a stable code, the field (for example `contributions[1].title`) and a sentence for the author. There are 26 codes (`ManifestRule`).
- **Schema:** `docs/plugin-api/manifest.schema.json`, generated with `schemars` from the same types. `mise run check` fails if it is stale.
- **Author check:** `infrawerf-plugin check <manifest or folder> [--json]`. Exit 0 valid, 1 not valid, 2 nothing checked.
- **Reference:** `docs/plugin-api/README.md` documents every field, extension point, capability and error code, and the compatibility rule.
- Nothing a plugin ships is executed. A valid manifest grants nothing. The application does not load manifests yet; that is F20/F22.

## Decisions made in this ticket

| Decision | Reason |
| --- | --- |
| JSON, not TOML or YAML | The ticket requires a published JSON Schema; one format for file and schema, and `serde_json` is already a dependency. |
| A manifest may declare every extension point | The registry already defines them. How content is supplied behind a contribution is F21 and later, and will raise the API version. |
| No plugin may request `interactive_terminal` | D055: no plugin gets a raw SSH session. The code is `capability_not_available`. `elevate` may be requested; D055 makes it an off-by-default exception the owner grants. |
| A requested capability must be used by a contribution | A plugin cannot ask for more than it declares a use for. |
| IDs starting with `builtin` or `infrawerf` are reserved | A plugin cannot pass itself off as part of InfraWerf. |
| Plugin ID 63 characters at most | With a dot and a 32-character contribution ID, the full ID fits the registry's 96-character limit. |
| New dependency `schemars` 1.2.3, pinned | Generates the schema from the types. It was already in `Cargo.lock` through Tauri; the lockfile gained `schemars_derive` and `serde_derive_internals`. |

None of these grants a capability that SCOPE.md forbids, so the stop condition was not reached.

## Checks on 8c908f4

| Check | Result |
| --- | --- |
| `mise run check` (now includes the schema check) | pass |
| `mise run test:feature -- F19` | pass |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 101 passed, 2 ignored (real-SSH fixture tests) |
| Release build | pass |
| `check_tracker.py --self-test` | pass |

Native suites were not run. No application behaviour changed: the new module is not yet called by the application, and the only change to existing code is documentation comments and a derive on two enums. The TypeScript contracts are unchanged.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| A valid manifest loads; each invalid fixture is rejected with a specific, stable error code | `tests/plugin_manifest.rs`: two valid fixtures load, register in the real extension registry and round-trip. Twenty-four invalid fixtures are each named after their code and must produce exactly that one code; the two size rules use generated input. The test fails if any of the 26 codes has no proof. |
| The published schema is generated from the same types the core validates with | The schema is `schemars::schema_for!(PluginManifest)`. A test compares the published file with a fresh generation and checks that the validator's field tables equal the schema's properties and required lists. `mise run check` runs `infrawerf-plugin schema --check`. |
| A manifest that requires a newer API version is refused with a clear message | Test `a_newer_api_version_is_refused_with_a_clear_message`. Command-line output: `newer_api_version  api_version  This plugin needs plugin API v2. This InfraWerf supports up to v1. Update InfraWerf to use it.` |
| The compatibility rule for API changes is written down | `docs/plugin-api/README.md`, "Compatibility rule" (six points), and the module comment in `plugin.rs`. |

## Notes

- Found while trying the tool: the schema allows `null` for optional fields but the validator refused it, and an unreadable file was reported as `malformed_json`. Both were fixed before the commit; null is accepted for optional fields and an unreadable file exits 2 without a manifest code.
- The schema marks optional fields as string-or-null and does not encode length or pattern rules; those are enforced by the validator and described in the reference.
- Review: no separate review pass was run for this ticket.
