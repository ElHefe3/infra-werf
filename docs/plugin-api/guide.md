# Writing an InfraWerf plugin — author guide

This guide takes you from an empty folder to a plugin manifest that InfraWerf accepts. It builds the example in [`examples/backup-overview`](../../examples/backup-overview). The exact rules for every field are in the [manifest reference](README.md).

## What the plugin API covers

Read this first, so you know what you are building.

- **The manifest comes first.** A plugin is a folder with one file, `infrawerf-plugin.json`, that says who the plugin is, what it adds to InfraWerf and what it asks permission to use.
- **Content so far is data.** From API v2 a `collector` or a `health_check` can carry a read-only `command`, a `collector` can describe a `workload_kind`, a `server_badge` can show a command's result with `badge`, a `remote_app` names an `app` and a `relationship_kind` gives a `relationship` style; that is a pack, described in [packs](packs.md). There is not yet a way to ship the code or data behind a tab, a badge or a menu entry. This guide builds a v1 manifest, the base every later version builds on.
- **InfraWerf finds plugins and lets the owner review them.** A plugin folder placed in the owner's plugins folder is listed in Settings, where the owner reads what it asks for and decides what to allow. A plugin without a `command` does not yet add anything visible once allowed; a pack's commands appear in each allowed server's Plugin data tab. Installing from a file or an address is not built yet.
- **A manifest grants nothing.** It is a request. Each owner decides what their own InfraWerf allows.

## Before you start

You need the InfraWerf repository checked out and its tools installed (`mise run doctor` passes). All commands below run from the repository root.

Build the check tool once:

```
mise run exec -- cargo build -p atlas-core --bin infrawerf-plugin
```

Then check a manifest with:

```
target\debug\infrawerf-plugin.exe check <path>
```

`<path>` is either the manifest file or the folder that holds it. Your plugin folder can be anywhere; it does not have to be inside the repository. If you give the file itself, its name is not checked, so make sure the file is called `infrawerf-plugin.json` before you share the plugin.

The tool only reads the manifest. It never runs anything in your plugin folder.

You can also run it without building first, as `mise run exec -- cargo run -p atlas-core --bin infrawerf-plugin -- check <path>`. That works the same, but when a manifest is refused it adds two lines of its own after the tool's output (`error: process didn't exit successfully` and `ERROR task failed`). Those only mean the tool reported problems; they are not a second error.

## Step 1 — Identity and a first contribution

Create a folder for your plugin and, inside it, a file named `infrawerf-plugin.json`:

```json
{
  "id": "org.example.backup-overview",
  "name": "Backup overview",
  "version": "0.1.0",
  "api_version": 1,
  "capabilities": ["read_saved_state"],
  "contributions": [
    {
      "id": "last-backup",
      "point": "server_badge",
      "title": "Last backup",
      "capabilities": ["read_saved_state"]
    }
  ]
}
```

What each part means:

- **`id`** identifies your plugin for ever. Use a domain you control, reversed, then the plugin's name: `org.example.backup-overview`. At least three parts, lower-case, joined by dots. You may not start it with `builtin` or `infrawerf`.
- **`name`** is what people see. 40 characters at most.
- **`version`** is your plugin's own version, as Semantic Versioning: `0.1.0`, `1.4.2`, `2.0.0-beta.1`.
- **`api_version`** is the InfraWerf plugin API you wrote the plugin for. Use `1`.
- **`capabilities`** lists everything the plugin asks to use. Here it is one thing: reading what InfraWerf has saved on the owner's PC.
- **`contributions`** lists what the plugin adds. Each has:
  - an **`id`** that is unique inside your plugin (`last-backup`),
  - a **`point`**, the place in InfraWerf it attaches to (`server_badge` is a badge on a server's card on the map),
  - a **`title`** people see,
  - the **`capabilities`** that this one contribution uses.

Check it:

```
target\debug\infrawerf-plugin.exe check path\to\your-plugin
```

```
org.example.backup-overview 0.1.0 is valid for plugin API v1 (1 contribution, 1 capability requested).
```

## Step 2 — Add a tab, and read an error

Now add a Backups tab to each server's workspace. It needs to run an approved read-only command over the owner's SSH access, so it uses two more capabilities. Add a second contribution:

<!-- expect: capability_not_requested -->
```json
{
  "id": "org.example.backup-overview",
  "name": "Backup overview",
  "version": "0.1.0",
  "api_version": 1,
  "capabilities": ["read_saved_state"],
  "contributions": [
    {
      "id": "last-backup",
      "point": "server_badge",
      "title": "Last backup",
      "capabilities": ["read_saved_state"]
    },
    {
      "id": "backups-tab",
      "point": "workspace_tab",
      "title": "Backups",
      "order": 200,
      "capabilities": ["read_saved_state", "ssh_session", "run_approved_command"]
    }
  ]
}
```

Replace the file's contents with this and check it again. This time it is refused, which is expected:

```
Not a valid manifest for plugin API v1: 2 problems.
  capability_not_requested  contributions[1].capabilities[1]  "ssh_session" is not in the manifest's capabilities list.
  capability_not_requested  contributions[1].capabilities[2]  "run_approved_command" is not in the manifest's capabilities list.
```

Each line has three parts:

1. **A code** (`capability_not_requested`). Codes are stable, so you can search for them. They are all listed in the [reference](README.md#error-codes).
2. **The field** (`contributions[1].capabilities[1]`): the second contribution, the second entry in its capabilities. Counting starts at 0.
3. **A sentence** saying what is wrong.

All problems are reported together, so you can fix them in one pass.

The rule you hit: the top-level `capabilities` list is the complete list of what the plugin asks the owner for. A contribution cannot use something that is not on it. The rule works the other way too: you cannot list a capability at the top that no contribution uses (`unused_capability`). The owner sees exactly what is used, no more and no less.

## Step 3 — Finish the manifest

Add the two capabilities to the top-level list, describe the plugin and set the version for release. Check it once more:

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
      "id": "last-backup",
      "point": "server_badge",
      "title": "Last backup",
      "capabilities": ["read_saved_state"]
    },
    {
      "id": "backups-tab",
      "point": "workspace_tab",
      "title": "Backups",
      "order": 200,
      "capabilities": ["read_saved_state", "ssh_session", "run_approved_command"]
    }
  ]
}
```

```
org.example.backup-overview 1.0.0 is valid for plugin API v1 (2 contributions, 3 capabilities requested).
```

The four new fields are optional:

- **`description`**: one line, 280 characters at most.
- **`author`**: 80 characters at most.
- **`license`**: preferably an SPDX identifier such as `MIT`.
- **`homepage`**: must start with `https://`. InfraWerf shows it and never fetches it.

**`order`** on the tab is also optional. It positions a contribution among others at the same point; lower comes first. Without it a contribution gets 1000, which places it after everything built in.

This manifest is identical to [`examples/backup-overview/infrawerf-plugin.json`](../../examples/backup-overview/infrawerf-plugin.json).

## Choosing extension points

A contribution's `point` decides where it appears and which capabilities it may use.

| `point` | Where it appears | Capabilities it may use |
| --- | --- | --- |
| `workspace_tab` | A tab in a server's workspace. | Any a plugin may request. |
| `settings_section` | A section of a server's Settings tab. | Any a plugin may request. |
| `server_badge` | A badge on a server's card on the map. | `read_saved_state` |
| `menu_item` | An entry in the main menu. | `read_saved_state` |
| `app_settings_section` | A section of the application Settings screen. | `read_saved_state`, `edit_map` |
| `collector` | Reads something from a server with a `command` ([packs](packs.md)). | `ssh_session`, `run_approved_command` |
| `health_check` | A check of a server: an unauthenticated probe, or from API v2 a `command`. | `network_probe`; from v2 also `ssh_session`, `run_approved_command` |
| `operation` | A brokered operation on a server. | `ssh_session`, `run_approved_command`, `read_files`, `write_files`, `transfer_files`, `elevate`, `control_workloads`, `loopback_tunnel` |
| `remote_app` | From API v2: an application in a server's Apps list ([packs](packs.md#applications)). | `ssh_session`, `loopback_tunnel` |
| `relationship_kind` | From API v2: a kind to choose when documenting a relationship on the map ([packs](packs.md#relationship-kinds)). | `read_saved_state` |

Using a capability a point does not allow is refused with `capability_not_permitted`. A badge, for example, may only read saved state; it cannot contact a server.

## Choosing capabilities

Ask for the least that works. Each capability is something the owner has to agree to.

| Capability | What it lets a contribution do |
| --- | --- |
| `read_saved_state` | Read the saved map, approved inventory and activity on the owner's PC. |
| `edit_map` | Change map entries on the owner's PC. Never changes a server. |
| `network_probe` | Unauthenticated TCP/HTTP reachability checks. |
| `ssh_session` | Use the owner's existing OpenSSH access through InfraWerf. A plugin never receives keys or a raw session. |
| `run_approved_command` | Run a fixed, approved read-only command in an authenticated session. |
| `read_files` | Read remote files and metadata over SFTP. |
| `write_files` | Create, rename, move or delete remote files after confirmation. |
| `transfer_files` | Upload and download between the owner's PC and a server. |
| `elevate` | Ask for sudo for one named action. |
| `control_workloads` | Start, stop or restart a discovered workload after confirmation. |
| `loopback_tunnel` | Open a loopback-only SSH forward to a server's own localhost. |

One capability exists in InfraWerf that no plugin may request: `interactive_terminal`. The terminal is the owner's own session. Asking for it is refused with `capability_not_available`.

## What the owner sees

The manifest is written for two readers: InfraWerf and the person deciding whether to trust your plugin.

In InfraWerf, **Settings → Plugins** lists every plugin folder found in the owner's plugins folder (the path is shown there). A folder whose manifest is not valid is listed separately with the same codes the check command prints. Each valid plugin has one page, and that page shows:

- your `name`, `version`, `description`, `author`, `license`, `homepage` and `id`;
- **What it adds:** each contribution's `title` and where its `point` puts it, and for a pack the exact `command` it runs;
- **What it asks for:** each capability as a sentence, marked as reading, changing or sudo, with the titles of the contributions that use it;
- **Trust level:** read-only, changes with confirmation, or trusted automation. The owner must pick one to enable the plugin; nothing is preselected. Read-only refuses every changing capability even if it was allowed on a server;
- **Servers:** for each server on the owner's map, which of your capabilities are allowed there. Nothing is allowed until the owner ticks it.

What follows from that for you:

- A found plugin can do nothing. It needs a trust level and at least one server.
- Each capability is allowed or refused per server. Do not assume that having one means having another.
- Sudo (`elevate`) can be requested but cannot be granted yet, so it is always refused.
- If your manifest changes after the owner enabled the plugin, the plugin is switched off until the owner reviews it and enables it again.
- Every refused use is written to the owner's activity history with your plugin's name.

Write your `name`, `description` and each `title` for the person reading that page.

## Versions and compatibility

- **Your version** (`version`) is yours. Raise it whenever you release a change.
- **The API version** (`api_version`) is InfraWerf's. Keep it at the version you wrote for. An InfraWerf that only knows an older API refuses your plugin with `newer_api_version` and tells the owner to update; it never loads a plugin while ignoring parts it does not understand.
- **Unknown fields are refused**, not ignored (`unknown_field`). Do not add your own fields to the manifest.
- **Keep your `id`.** A different ID is a different plugin.

The full rule is in the [reference](README.md#compatibility-rule).

## Checklist before you share a plugin

1. `check` reports the manifest as valid.
2. The `id` starts with a domain you control.
3. Every capability is one you need, and the description says what the plugin does with it.
4. Titles and the description read well to someone who has not seen your code.
