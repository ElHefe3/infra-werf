# What InfraWerf keeps stable

InfraWerf is pre-1.0: its own version number says nothing about compatibility yet. What plugin authors and other tools can rely on is a small set of versioned interfaces, each with its own number and its own rule.

## Versioned interfaces

| Interface | Current version | Accepted | Its rule |
| --- | --- | --- | --- |
| Plugin manifest (`infrawerf-plugin.json`), including pack commands, apps, badges, relationship kinds, views and jobs | plugin API 2 | 1 and 2 | [Compatibility rule](plugin-api/README.md#compatibility-rule) |
| Plugin logic: the WebAssembly module contract and its host functions | sandbox contract 1 | 1 | [Plugin logic](plugin-api/logic.md) |
| Plugin views: the message API between a view and InfraWerf | view message API 1 | 1 | [Versioning](plugin-api/views.md#versioning) |
| Plugin packages: folders, `.zip` and `.tar.gz` archives, SHA-256 and minisign signatures | as documented | — | [Sharing and installing](plugin-api/install.md) |

In short:

- **A manifest means what its `api_version` says.** InfraWerf accepts every API version from the oldest it supports to the newest it knows, and reads each manifest exactly as its version defines. A plugin written for a newer API is refused with `newer_api_version`; it is never loaded with unknown parts ignored.
- **New names raise the version.** Any new field, extension point, capability or host function raises the plugin API version, because unknown names are refused rather than ignored.
- **Nothing is removed silently.** Support for an old API version is dropped only in a release whose notes say so, and never in the release that introduces its replacement.
- **Error codes keep their meaning** for as long as the API version they appeared in is accepted. Messages may become clearer.

Until InfraWerf's first public release, plugin API 2 is still being extended in place: names are added to it, never changed or removed. From that release on, the rule above applies in full, and the next addition raises the version to 3.

## Not an interface

These can change in any release, without notice:

- the commands between InfraWerf's interface and its Rust core, and `ui/contracts.gen.ts`;
- the SQLite database and its schema (InfraWerf migrates its own data forward; read it only through InfraWerf);
- the interface's layout, wording and styles;
- anything under `scripts/`, `agent/` and the test fixtures.

## Plugins and licences

InfraWerf is licensed under MIT OR Apache-2.0. A plugin talks to InfraWerf only through its manifest, its sandboxed module and its view's message API, so a plugin is its author's own work under the licence its author chooses. Name it in the manifest's `license` field, preferably as an SPDX identifier. InfraWerf shows it to the owner and enforces nothing about it.
