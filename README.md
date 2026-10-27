# InfraWerf

Your homelab, connected.

A native infrastructure map and server workspace, with a Windows desktop as the release target. A separate compact Android interface and phone pairing are future S09 work.

**Stack:** **React + TypeScript + React Flow, Tauri on Windows and native Android, Rust, SQLite and system OpenSSH/SSH agent**, with SFTP first and rsync later. The desktop map now uses React Flow and real Rust persistence. See [STACK.md](STACK.md) for the original source and requirements.

## Agent and delivery tracker

Start with [.agent.md](.agent.md) or [the main agent guide](agent/AGENT.md). The guide contains the roadmap and links to 33 tickets grouped into 15 features and 9 scopes. [Product scope](agent/SCOPE.md) defines 25 core tickets and 8 deferred tickets. T032/T033 separate desktop guard/SSH verification from later paired integration without expanding product scope. [Discoveries](agent/discovery/README.md) and [the checkpoint](agent/evidence/CHECKPOINT.md) preserve findings and efficient restart context.

The current release target is the Windows desktop. Phone controls over LAN or VPN, authenticated pairing and Android packaging remain future S09 work. [BUILDING.md](BUILDING.md) records development and packaging commands; [F02 acceptance](agent/evidence/F02-acceptance.md) records the original data-preservation proof.

## Native Windows desktop

Open the installed primary app from **InfraWerf** in the Start menu, **InfraWerf Desktop.cmd**, or `mise run start:desktop`. Build its installer with `mise run build:desktop`; [the release guide](agent/RELEASE.md) explains installation, data, updates and recovery. The app runs without Python or a development server. `mise run dev:desktop` starts a development window; `mise run smoke:desktop` builds a debug executable.

The installed release opens the existing native store and does not automatically import or replace it. T026 verified its allowlisted data and recovery copy, and the owner accepted the installed desktop as primary. InfraWerf 0.1.2 fixes the reported SSH/inventory compatibility issues and has [complete desktop regression evidence](agent/evidence/DESKTOP-RETEST-acceptance.md). The prior Atlas installation and obsolete shortcuts have been removed. The original `data/atlas.sqlite3` and native recovery snapshot remain data fallbacks, with the established application identifier and data location preserved.

Desktop capabilities include editable map entries and relationships, saved inventory and search, strict-trust SSH verification, protected SFTP Files and Transfers, interactive Terminal, health, workload inspection/logs and scoped service/container control. Saved observations do not imply current reachability. Protected operations require an authenticated SSH session; no phone coordinator is active.

`mise run test:desktop` exercises the **actual Tauri WebView2/Rust IPC** against synthetic data, including restart and failure paths. Build first. Private test databases/screenshots/results remain under ignored `.tools/`.

Open a server → **Settings → Verify SSH access** to check its concrete alias using the PC's system OpenSSH configuration and existing public-key identities/agent. InfraWerf requires strict host trust and shows the resolved endpoint/fingerprint; it never copies private keys or changes known-host files. Unknown/changed fingerprints must be verified independently and resolved through your existing SSH trust workflow. A completed verification is a short check, not an open protected session. Direct aliases are supported; jump/proxy/known-host helper configurations are explicitly unavailable. Files, terminal, transfers and scoped workload actions require a separately authenticated session.

Controlled SSH proof: `mise run ssh:fixture` prepares a private pinned test runtime; `mise run test:ssh` tests real OpenSSH against loopback servers; `mise run test:ssh-native` exercises the actual Settings UI (build first). Test-only keys/data stay ignored; no real hosts or trust files are changed.

## Python reference

Python 3.11 or newer; no third-party packages required.

```powershell
python atlas.py --inventory-dir '..\server map\dashboard\data'
```

- Desktop: http://127.0.0.1:8770/
- Mobile layout: http://127.0.0.1:8770/mobile

`Start-Atlas.ps1` locates the bundled Codex Python runtime when Python is not on PATH. The coordinator stays in the terminal; Ctrl+C stops it. Port 8770 is separate from the existing infrastructure progress dashboard.

To keep Atlas running after the launching terminal or chat turn closes, run `./Start-Atlas.ps1 -Background`. This starts a hidden Python process with logs in `data/server.stdout.log` and `data/server.stderr.log`, and records its process ID in `data/server.pid`. It does not install a startup service or survive a PC reboot.

The server currently binds to loopback only. The mobile interface can be reviewed on this computer at a phone viewport. Access from a physical phone requires a later authenticated network-access increment; the hosted prototype remains available separately. This build is not uploaded to Sites and private inventory is not published there.

## Python reference features

- Shared HTTP API and persistent SQLite storage at `data/atlas.sqlite3`.
- Add and rename hosts; update their connection hostname.
- Drag desktop nodes and save positions; mobile uses a compact independent arrangement.
- Create and remove documented directional relationships.
- Search hosts, services and containers; browse full saved inventory in a server workspace.
- Import the existing four sanitized dashboard records on startup or with **Import newer saved reports**. Records are read, never changed. No host connections are made.
- Recorded activity and 15-second refresh of coordinator state across interfaces.
- Input validation, bounded metadata ingestion, local host/origin checks and explicit static assets.

Observation times are preserved. Reports older than 15 minutes are labelled stale. Stale means current health is unknown, not offline. No fake CPU/memory/disk utilisation is displayed. Services show their saved state and do not imply current health. Relationships start empty because prototype sample dependencies were not verified infrastructure facts.

## Data

The database contains hostnames, map coordinates, documented relationships and selected inventory metadata, but no passwords, SSH private keys or raw configurations. `data/` is ignored by Git. Back up the database after stopping the coordinator (SQLite's backup API is an alternative for a running process). Deleting it discards the saved map. The source inventory directory remains independent.

Import is atomic. Invalid reports leave the database unchanged; older or identical reports are ignored. Updates preserve server IDs, owner-edited names, connection hostnames, map positions and relationships. Imported observed hostname is displayed separately from the connection hostname.

## Verify

```powershell
python -m unittest discover -s tests -v
```

Tests cover persistence across coordinator recreation, ingestion ordering and atomicity, input validation, shared API/assets and rejection of foreign-origin requests/private file access. Browser checks are recorded in `IMPLEMENTATION.md`.

## Next increments

See [ROADMAP.md](ROADMAP.md). The Python reference is a separate local prototype; its data does not sync automatically with the native application. Release acceptance and owner cutover are tracked in [T026](agent/features/F12-release-acceptance.md); mobile, pairing, agents and rsync remain deferred.
