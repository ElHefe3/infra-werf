# InfraWerf documentation

The pages are grouped by who they are for: owners, plugin authors and developers.

## For owners (using InfraWerf)

| Page | What it covers |
| --- | --- |
| [Install](install.md) | Installing, first launch, updating, uninstalling. |
| [User guide](user-guide.md) | Each screen and action, from the map to scoped service control. |
| [Security model](security.md) | What InfraWerf reads, what it changes, how sessions and Lock work. |
| [Data and backup](data-and-backup.md) | Where your data lives, how to back it up, restore or reset it. |
| [Troubleshooting](troubleshooting.md) | Symptoms, causes and fixes, organised by what you see. |
| [Plugins](plugins.md) | Installing, reviewing and removing plugins. |

## For plugin authors

| Page | What it covers |
| --- | --- |
| [Plugins overview](plugins.md#for-authors) | What a plugin can add and where to start. |
| [Author guide](plugin-api/guide.md) | Build a plugin from an empty folder. |
| [Manifest reference](plugin-api/README.md) | Every field of `infrawerf-plugin.json`. |
| [Packs](plugin-api/packs.md) | Read-only commands, extractors, workload kinds, badges and applications. |
| [Views](plugin-api/views.md) | A plugin's own page inside InfraWerf. |
| [Plugin logic](plugin-api/logic.md) | Sandboxed WebAssembly jobs. |
| [Sharing and installing](plugin-api/install.md) | Archives, hashes and signatures. |
| [Compatibility](COMPATIBILITY.md) | What InfraWerf promises to keep stable. |

## For developers

| Page | What it covers |
| --- | --- |
| [Build from source](build.md) | Toolchain, first build, day-to-day commands, release builds. |
| [Testing](testing.md) | What each test proves and which ones need a real Windows session. |
| [Contributing](../CONTRIBUTING.md) | Pull request rules and the owner-safety rules. |
| [Native development notes](../BUILDING.md) | Detailed pins, Android notes and the full setup history. |
| [Release process](../agent/RELEASE.md) | Building the installer and its artifact checks. |
| [Project tracker](../agent/AGENT.md) | Scopes, features, tickets and evidence. |

## Reference

- [Roadmap](../ROADMAP.md)
- [Stack and requirements](../STACK.md)
- [Brand guide](../agent/BRAND.md)
- [Python reference prototype](data-and-backup.md#the-python-reference)
