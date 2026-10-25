# Atlas

A native infrastructure map and server workspace, with a Windows desktop and a separate compact Android interface planned on the shared Rust domain.

**Stack:** **React + TypeScript + React Flow, Tauri on Windows and native Android, Rust, SQLite and system OpenSSH/SSH agent**, with SFTP first and rsync later. The desktop map now uses React Flow and real Rust persistence. See [STACK.md](STACK.md) for the original source and requirements.

## Agent and delivery tracker

Start with [.agent.md](.agent.md) or [the main agent guide](agent/AGENT.md). The guide contains the roadmap and links to 31 tickets grouped into 14 features and 8 scopes. [Product scope](agent/SCOPE.md) defines the 26-ticket core release and five deferred agent/rsync tickets. [Discoveries](agent/discovery/README.md) and [the checkpoint](agent/evidence/CHECKPOINT.md) preserve findings and efficient restart context.

The core is native Windows/Android with full phone controls through the Windows PC over LAN or an existing VPN. S01 foundations are complete. S02 prioritises the Windows map, saved-inventory workspace and native lifecycle; the separate compact phone composition remains T009. SSH, authenticated phone access and transfers are later tickets and show as unavailable. [BUILDING.md](BUILDING.md) records commands and [F02 acceptance](agent/evidence/F02-acceptance.md) records preservation proof.

## Native Windows desktop

Build with `mise run smoke:desktop`, then open **Atlas Desktop.cmd** or run `mise run start:desktop`. The built application runs without Python or a development server. `mise run dev:desktop` starts the native development window.

The launcher supplies the reference database for a **one-time read-only copy** into Atlas's own application-data folder. The original `data/atlas.sqlite3` stays available to the Python reference; a private legacy recovery copy is retained beside the new native database. Existing native data wins on subsequent launches. This is separate storage, not cross-application sync.

Desktop capabilities: add/edit/remove map entries, drag positions, directed labelled relationships, host/workload search, full saved service/container inventory, inspector/activity, and keyboard-accessible workspace tabs. Saved observation state never implies current reachability. Files/Terminal/Transfers and SSH controls remain unavailable until their respective tickets pass. The phone coordinator has no listener yet.

`mise run test:desktop` exercises the **actual Tauri WebView2/Rust IPC** against synthetic data, including restart and failure paths. Build first. Private test databases/screenshots/results remain under ignored `.tools/`.

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

See [ROADMAP.md](ROADMAP.md). Remote authentication, SSH sessions, files, terminal, transfers, agents and service control are deliberately not connected yet. Workspace tabs state that limitation; none simulate successful authentication or operations.
