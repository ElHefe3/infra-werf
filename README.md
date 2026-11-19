# InfraWerf

**Your homelab, connected.**

A desktop workspace for the servers you run yourself. See your machines on one map, and open any of them to review its inventory, health, files, terminal, services and containers, all through your own OpenSSH access.

[![Platform](https://img.shields.io/badge/platform-Windows%2011%20x64-0078D4)](docs/install.md)
[![Licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-green)](#licence)
[![Stack](https://img.shields.io/badge/stack-Tauri%20%C2%B7%20Rust%20%C2%B7%20React%20%C2%B7%20SQLite-orange)](docs/build.md)
[![Plugin API](https://img.shields.io/badge/plugin%20API-v2-blueviolet)](docs/plugins.md)

> **No account. No cloud service. No telemetry.** Your instance, its map and its saved data stay on your PC. InfraWerf connects only to the servers you configure.

---

## Why InfraWerf

- **One map for the whole estate.** Draw how your machines depend on each other and keep it as you change things.
- **Your access, not a new one.** It uses the OpenSSH setup and keys you already have. It never copies a private key and never edits your known-hosts file.
- **Honest about what it knows.** Every saved observation shows when it was taken. Old data is labelled stale, never presented as live.
- **Deliberate changes.** Anything that changes a server needs a fresh, separate authentication, shows an exact preview, and asks for confirmation.
- **Extensible within limits.** Plugins add data, views and automation. A manifest is a request, and you decide what each plugin may do.

## Features

| Area | What you get |
| --- | --- |
| **Map** | Editable server cards, directed relationships, pan, zoom, keyboard navigation and automatic layout. |
| **Server workspace** | Saved inventory with search, the server's current health and its workloads. |
| **Health** | Bounded TCP/HTTP checks with clear *healthy*, *unhealthy*, *stale* and *unknown* states. |
| **Files** | Protected SFTP browser: browse, create folders, rename, move and delete, each with an exact-path preview and confirmation. |
| **Transfers** | Streamed uploads and downloads with SHA-256 checks, progress, cancel and no silent overwrites. |
| **Terminal** | Interactive shell on an authenticated session, with resize and Ctrl+C. |
| **Services and containers** | Inspect systemd, Podman and Docker workloads and their logs; start, stop and restart through reviewed, scoped actions. |
| **Apps** | Open a web app running on a server's localhost through a private, short-lived tunnel. |
| **Plugins** | Install from a folder, an archive or an `https://` address. Each plugin declares what it needs; nothing is enabled until you allow it. |
| **Lock** | One action clears every protected result and session from memory. |

## Quick start

### Install (Windows 11 x64)

1. Download the latest `InfraWerf_<version>_x64-setup.exe` from the releases you were given.
2. Run it. It installs for your user only, with no administrator prompt.
3. Open **InfraWerf** from the Start menu.

Full details, including updates, uninstall and data locations: **[docs/install.md](docs/install.md)**.

### First server

1. Add a server on the map and give it the hostname it uses in your OpenSSH config.
2. Open the server, go to **Settings → Verify SSH access**.
3. Confirm the fingerprint against your own records, then continue.

Step-by-step guidance for each screen: **[docs/user-guide.md](docs/user-guide.md)**.

### Build from source (developers)

```bash
mise trust
mise install
mise run deps
mise run check
mise run smoke:desktop
```

Requires [mise](https://mise.jdx.dev/), the Microsoft C++ Build Tools and WebView2. See **[docs/build.md](docs/build.md)**.

## Documentation

| I want to… | Read |
| --- | --- |
| Install it and get started | [Install](docs/install.md) · [First steps](docs/user-guide.md#first-steps) |
| Learn every screen and action | [User guide](docs/user-guide.md) |
| Understand what it can and cannot touch | [Security model](docs/security.md) |
| Back up, move or reset my data | [Data and backup](docs/data-and-backup.md) |
| Fix a problem | [Troubleshooting](docs/troubleshooting.md) |
| Build or develop InfraWerf | [Build from source](docs/build.md) · [Testing](docs/testing.md) |
| Write or install a plugin | [Plugins](docs/plugins.md) · [Plugin author guide](docs/plugin-api/guide.md) · [Plugin API reference](docs/plugin-api/README.md) |
| Know what stays stable between versions | [Compatibility](docs/COMPATIBILITY.md) |
| Contribute | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Report a security issue | [SECURITY.md](SECURITY.md) |

The full index is in **[docs/README.md](docs/README.md)**.

## Status

- **Windows 11 x64 desktop** is the release target, in daily use, and accepted by its owner as the primary app. Current version: **0.1.13**.
- **Phone access and Android** (compact interface, authenticated pairing over LAN or VPN) are planned, not part of this release.
- **Python reference** (`atlas.py`) is a retired prototype kept for comparison. The desktop app does not read or write its data. See [docs/data-and-backup.md](docs/data-and-backup.md#the-python-reference).
- **Code signing** is not yet configured. Windows may warn on first run.

Project history, decisions and acceptance evidence live under [`agent/`](agent/AGENT.md) and [`docs/`](docs/README.md). The [roadmap](ROADMAP.md) lists what comes next.

## Contributing

Contributions are welcome. The most useful first step is a plugin: most new features belong in one, in your own repository. Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request.

## Licence

InfraWerf is licensed under either of

- [MIT licence](LICENSE-MIT)
- [Apache licence, version 2.0](LICENSE-APACHE)

at your option. Plugins are their authors' own works under the licences their authors choose.
