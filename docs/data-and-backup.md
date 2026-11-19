# Data and backup

## Where your data lives

| Data | Location |
| --- | --- |
| Native workspace (map, servers, relationships, saved inventory, settings, plugin records) | `%APPDATA%\dev.atlas.workspace\atlas.sqlite3` |
| Recovery copies from earlier migrations | `%APPDATA%\dev.atlas.workspace\legacy-recovery-*.sqlite3`, if present |
| Application files | `%LOCALAPPDATA%\InfraWerf` |
| Plugins | The plugins folder, shown in **Settings → Plugins** |
| Python reference database | `data\atlas.sqlite3` in the source repository (developers only) |

The folder name `dev.atlas.workspace` predates the InfraWerf name. It is kept so that existing data stays where it was.

**What is stored:** server names, connection hostnames, map positions, relationships, saved inventory and observations with their timestamps, plugin install records and permissions, and settings.

**What is not stored:** passwords, private keys, raw configuration files, terminal output, or file contents.

## Back up

Back up while InfraWerf is closed, so the database is not being written.

1. Close InfraWerf.
2. Copy the whole `dev.atlas.workspace` folder to a safe place, including any `legacy-recovery-*.sqlite3` files and the plugins folder.
3. Keep the copy somewhere you control, for example an encrypted drive.

```powershell
# Adjust the destination. Run with InfraWerf closed.
Copy-Item "$env:APPDATA\dev.atlas.workspace" "D:\Backups\InfraWerf-$(Get-Date -Format yyyy-MM-dd)" -Recurse
```

Make a backup before every update, and before uninstalling with **Delete app data**.

## Restore

1. Close InfraWerf.
2. Rename the current `%APPDATA%\dev.atlas.workspace` folder, so nothing is overwritten.
3. Copy your backup folder back to `%APPDATA%\dev.atlas.workspace`.
4. Start InfraWerf and check the map and a server's inventory.

## Reset

To start again, close InfraWerf, then move or delete `%APPDATA%\dev.atlas.workspace`. The next launch creates an empty workspace. This does not change any server.

Uninstalling with **Delete app data** does the same thing.

## Saved data is a snapshot

Everything InfraWerf shows about a server, apart from a live check, is what it saved at a specific time. Every saved observation shows its timestamp. Old observations are labelled **stale**. Stale is not offline.

## Updates keep your data

Updating the application replaces its program files only. Your data folder is not touched by the installer. Back up before each update anyway.

## Sharing or moving to a new PC

Copy the data folder and the plugins folder to the new PC, install InfraWerf, close it, then replace its new data folder with yours. Check each server's verification again on the new PC, because its OpenSSH configuration and trust may differ.

## The Python reference

The repository also contains a Python prototype (`atlas.py`) with its own database, `data\atlas.sqlite3`. It is a retired reference kept for comparison and fallback.

- The desktop app does **not** read or write that database.
- Changes do not sync between the two.
- Keep it as an independent copy if you still need it. Do not run both writers against the same file.

Its original run instructions are in the git history of `README.md`. They are not maintained for daily use.
