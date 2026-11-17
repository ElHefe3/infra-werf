# InfraWerf plugin API v2 — manifest reference

A plugin is a folder with one file that describes it: `infrawerf-plugin.json`. This page defines that file for plugin API version 2. InfraWerf also accepts manifests written for version 1; [what changed](#changes-in-v2) is listed below. The machine-readable form is [`manifest.schema.json`](manifest.schema.json), which is generated from the same Rust types InfraWerf validates with (`crates/atlas-core/src/plugin.rs`).

A valid manifest grants nothing. It states what the plugin adds and what it asks for; the owner of each InfraWerf instance decides what to allow. Validating a manifest never runs anything the plugin ships.

This is the reference. To write a plugin step by step, start with the [author guide](guide.md); the finished example is in [`examples/backup-overview`](../../examples/backup-overview). Commands that a plugin runs on a server are defined in [packs](packs.md).

## Example

```json
{
  "id": "org.example.backup-overview",
  "name": "Backup overview",
  "version": "1.0.0",
  "api_version": 1,
  "description": "Shows when each server was last backed up.",
  "author": "Example Author",
  "license": "MIT",
  "homepage": "https://example.org/backup-overview",
  "capabilities": ["read_saved_state", "ssh_session", "run_approved_command"],
  "contributions": [
    {
      "id": "backups-tab",
      "point": "workspace_tab",
      "title": "Backups",
      "order": 200,
      "capabilities": ["read_saved_state", "ssh_session", "run_approved_command"]
    },
    {
      "id": "last-backup",
      "point": "server_badge",
      "title": "Last backup",
      "capabilities": ["read_saved_state"]
    }
  ]
}
```

## Checking a manifest

```
mise run exec -- cargo build -p atlas-core --bin infrawerf-plugin
target\debug\infrawerf-plugin.exe check path\to\plugin
```

The path is the manifest or the folder that holds it. Add `--json` for machine-readable output. Build once; after that only the second line is needed.

| Exit code | Meaning |
| --- | --- |
| 0 | Valid for the plugin API version the manifest declares. |
| 1 | Not valid. Every problem is listed as `code  field  message`. |
| 2 | Nothing was checked: wrong usage, or the file could not be read. |

## Fields

Unknown fields are refused, so a field from a later API version is never silently ignored. An optional field may be left out or set to `null`.

| Field | Required | Rule |
| --- | --- | --- |
| `id` | yes | Reverse-domain identifier such as `org.example.backup-overview`. At least three segments separated by dots. Each segment is lower-case letters, digits and hyphens, starts with a letter and does not end with a hyphen. 63 characters at most. The first segment may not be `builtin` or `infrawerf`. The ID never changes between versions of a plugin. |
| `name` | yes | Display name, 1 to 40 characters, no leading, trailing or control characters. |
| `version` | yes | The plugin's version as [Semantic Versioning 2.0.0](https://semver.org/), for example `1.4.0` or `2.0.0-beta.1`. |
| `api_version` | yes | The plugin API version the plugin was written for. A positive whole number: `2` today, or `1` for a manifest that uses nothing added in v2. |
| `description` | no | One line, 1 to 280 characters. |
| `author` | no | 1 to 80 characters. |
| `license` | no | 1 to 64 characters, preferably an SPDX identifier. |
| `homepage` | no | An `https://` address, 200 characters at most. InfraWerf shows it and never fetches it. |
| `capabilities` | yes | Every capability the plugin asks for. No duplicates. Each must be used by at least one contribution. |
| `contributions` | yes | 1 to 64 contributions. |

Each contribution:

| Field | Required | Rule |
| --- | --- | --- |
| `id` | yes | Name within the plugin: lower-case letters, digits and hyphens, starting with a letter, 32 characters at most, unique in the manifest. The full identifier is `<plugin id>.<id>`. |
| `point` | yes | One of the extension points below. |
| `title` | yes | Display name, 1 to 40 characters. |
| `order` | no | Position among contributions at the same point, 0 to 65535. Defaults to 1000, after everything built in. |
| `capabilities` | yes | The capabilities this contribution uses. At least one, no duplicates, each listed in the manifest's `capabilities` and permitted at its extension point. |
| `command` | no | A read-only command for a `collector` or `health_check`, with `argv`, `parameters`, `extract` and `healthy`. Needs `api_version` 2. Defined in [packs](packs.md). |
| `workload_kind` | no | For a `collector` whose command reads a table: show its rows as workloads, with `name_column` and `state_column`. Needs `api_version` 2. Defined in [packs](packs.md#workload-kinds). |
| `app` | for `remote_app` | The application to open: `port` and `path` on the server's own localhost. Needs `api_version` 2. Defined in [packs](packs.md#applications). |
| `badge` | no | For a `server_badge`: the `observation` it shows, the `id` of a command in the same manifest. Needs `api_version` 2. Defined in [packs](packs.md#badges). |
| `relationship` | no | For a `relationship_kind`: its `line` and `color`. Needs `api_version` 2. Defined in [packs](packs.md#relationship-kinds). |
| `view` | no | For a `workspace_tab`: the plugin's own page, `entry`, shown in an isolated view. Needs `api_version` 2. Defined in [views](views.md). |
| `job` | for `job` | The plugin's own logic: its `module` and the `actions` it may ask for. Needs `api_version` 2. Defined in [plugin logic](logic.md#declaring-a-job). |

The file is UTF-8 JSON, 64 KiB at most.

## Extension points

A contribution may only use capabilities its extension point permits.

| `point` | What it adds | Capabilities permitted |
| --- | --- | --- |
| `workspace_tab` | A tab in a server's workspace. | Any a plugin may request. |
| `settings_section` | A section of a server's Settings tab. | Any a plugin may request. |
| `server_badge` | A badge on a server's card on the map. | `read_saved_state` |
| `menu_item` | An entry in the main menu that opens a view. | `read_saved_state` |
| `app_settings_section` | A section of the application Settings screen. | `read_saved_state`, `edit_map` |
| `collector` | Reads something from a server with a `command`. | `ssh_session`, `run_approved_command` |
| `health_check` | A check of a server: an unauthenticated probe, or from v2 a `command`. | `network_probe`; from v2 also `ssh_session`, `run_approved_command` |
| `operation` | A brokered operation on a server. | `ssh_session`, `run_approved_command`, `read_files`, `write_files`, `transfer_files`, `elevate`, `control_workloads`, `loopback_tunnel` |
| `remote_app` | From v2: an application in a server's Apps list, opened through a private tunnel to the server's own localhost. | `ssh_session`, `loopback_tunnel` |
| `relationship_kind` | From v2: a kind of relationship the owner can choose when documenting one on the map, with its own line style. | `read_saved_state` |
| `job` | From v2: the plugin's own logic, run in InfraWerf's sandbox when the owner starts it or on the owner's schedule. | `read_saved_state`, `ssh_session`, `run_approved_command`, `control_workloads`, `elevate` |

API v2 defines content for these points: a `collector` and a `health_check` may carry a `command`, a `collector` may also describe a `workload_kind`, a `server_badge` may show a command's result, a `remote_app` names an application and a `relationship_kind` names a line style ([packs](packs.md)), a `workspace_tab` may show a `view` ([views](views.md)) and a `job` runs a module ([plugin logic](logic.md)). The content behind the remaining points is defined by later parts of the API and will raise the API version.

## Capabilities

| Capability | Meaning |
| --- | --- |
| `read_saved_state` | Read the saved map, approved inventory and activity on this PC. |
| `edit_map` | Change map entries on this PC. Never changes a server. |
| `network_probe` | Unauthenticated TCP/HTTP reachability checks. |
| `ssh_session` | Use the owner's existing OpenSSH access through InfraWerf. The plugin never receives keys or a raw session. |
| `run_approved_command` | Run a fixed, approved read-only command in an authenticated session. |
| `read_files` | Read remote files and metadata over SFTP. |
| `write_files` | Create, rename, move or delete remote files after confirmation. |
| `transfer_files` | Upload and download between this PC and a server. |
| `elevate` | Ask for sudo for one named action. Off unless the owner allows it for one plugin, one server and one action. |
| `control_workloads` | Start, stop or restart a discovered workload after confirmation. |
| `loopback_tunnel` | Open a loopback-only SSH forward to a server's own localhost. |

`interactive_terminal` exists in InfraWerf but no plugin may request it: the terminal is the owner's own session.

## Error codes

Each problem has a code, the field it concerns and a sentence. Codes are part of the API: a code keeps its meaning for as long as the API version it appeared in is accepted. All problems in a manifest are reported together, except that problems with the file's structure (the first eight rows) are reported before the rest is examined.

| Code | Meaning |
| --- | --- |
| `malformed_json` | The file is not valid UTF-8 JSON. |
| `too_large` | The file is larger than 64 KiB. |
| `wrong_type` | A value has the wrong JSON type. |
| `unknown_field` | A field that is not part of this API version. |
| `missing_field` | A required field is absent. |
| `unknown_capability` | A capability name this API version does not have. |
| `unknown_point` | An extension point this API version does not have. |
| `invalid_api_version` | `api_version` is not a positive whole number. |
| `newer_api_version` | The plugin needs a newer plugin API than this InfraWerf has. |
| `invalid_id` | The plugin ID is not a reverse-domain identifier. |
| `reserved_id` | The plugin ID starts with `builtin` or `infrawerf`. |
| `invalid_name` | The name is empty, too long or has surrounding or control characters. |
| `invalid_version` | The version is not Semantic Versioning 2.0.0. |
| `invalid_text` | A description, author or licence is empty, too long or not on one line. |
| `invalid_url` | The homepage is not a plain `https://` address. |
| `duplicate_capability` | The same capability is listed twice in one list. |
| `capability_not_available` | A capability no plugin may request. |
| `unused_capability` | A requested capability that no contribution uses. |
| `no_contributions` | The plugin contributes nothing. |
| `too_many_contributions` | More than 64 contributions. |
| `invalid_contribution_id` | A contribution ID is not a lower-case name. |
| `duplicate_contribution` | Two contributions share an ID. |
| `invalid_title` | A title is empty, too long or has surrounding or control characters. |
| `contribution_without_capability` | A contribution lists no capability. |
| `capability_not_requested` | A contribution uses a capability the manifest does not request. |
| `capability_not_permitted` | A contribution uses a capability its extension point does not permit. |
| `invalid_command` | A `command` has an unknown field, a missing field or a wrong type. |
| `command_not_supported` | A `command` where none is possible: a point other than `collector` or `health_check`, or a manifest that declares API v1. |
| `command_needs_capability` | A contribution with a `command` does not list `ssh_session` and `run_approved_command`. |
| `unsafe_argument` | An `argv` entry has a character outside the safe set, the program is not a plain name or absolute path, or there are no entries or more than 16. |
| `forbidden_program` | The program is a shell, an interpreter, sudo, a network client or a program that changes a server. |
| `undeclared_parameter` | `argv` uses a `{name}` that `parameters` does not declare. |
| `unused_parameter` | A declared parameter that `argv` never uses. |
| `invalid_parameter` | A parameter name is malformed or repeated, or there are more than 4. |
| `invalid_extractor` | A line number, column list, skip count or JSON path is malformed. |
| `invalid_health_rule` | The `healthy` values are missing on a health check, present on a collector, used with a table, or malformed. |
| `invalid_app` | An `app` is missing on a `remote_app`, present elsewhere, has a field other than `port` and `path`, a port of 0, a path that is not a plain path on the server, or the contribution does not list `ssh_session` and `loopback_tunnel`. |
| `invalid_badge` | A `badge` is on another point, in a v1 manifest, or its `observation` is not a command in the same manifest that reads one value without parameters. |
| `invalid_relationship_kind` | A `relationship` is on another point, or has a line or colour that is not in the lists. |
| `invalid_workload_kind` | A `workload_kind` is not on a collector with a table extractor, or names a column the table does not have. |
| `invalid_view` | A `view` is on another point, in a v1 manifest, or its `entry` is not a plain path inside the plugin ending in `.html`. |
| `invalid_job` | A `job` is missing on a `job`, on another point or in a v1 manifest; its `module` is not a plain path inside the plugin ending in `.wasm`; an action's `id`, `title`, `control`, `target` or `effect` breaks its rule, or two share an `id`, or there are more than 16; or a job with actions does not list `ssh_session` and `control_workloads`. |

## Compatibility rule

The plugin API version is one whole number.

1. **When it increases.** The number goes up whenever a plugin written for the previous version could behave differently, or a manifest could mean something different. That includes every new manifest field, extension point or capability, because unknown names are refused rather than ignored.
2. **What does not increase it.** Clearer wording of a message, a new InfraWerf feature that plugins cannot see, and fixes that make InfraWerf match this document.
3. **What InfraWerf accepts.** Each InfraWerf release accepts every API version from its oldest supported version to its current one, and treats a manifest exactly as the version it declares defines. Today that is versions 1 and 2.
4. **Newer plugins.** A plugin that declares a newer API version than InfraWerf has is refused with `newer_api_version` and a message naming both versions. It is never loaded with unknown parts ignored.
5. **Retiring a version.** Support for an old API version is dropped only in a release whose notes say so, and never in the same release that introduces its replacement.
6. **The plugin's own version** (`version`) is separate. It follows Semantic Versioning and says nothing about API compatibility.

## Changes in v2

- A contribution may have a `command` ([packs](packs.md)), at the `collector` and `health_check` points only.
- A `health_check` may use `ssh_session` and `run_approved_command`. In a v1 manifest it may still use only `network_probe`.
- Two new extension points, `remote_app` and `relationship_kind`, and four new contribution fields: `workload_kind`, `app`, `badge` and `relationship` ([packs](packs.md)).
- A `workspace_tab` may carry a `view`, its own page in an isolated view ([views](views.md)).
- A new extension point, `job`, with its `job` field: the plugin's own logic ([plugin logic](logic.md)).
- Sixteen new error codes, from `invalid_command` to `invalid_job`.

Nothing was removed and no v1 rule changed, so a v1 manifest means what it always did.
