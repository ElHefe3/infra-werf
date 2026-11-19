# User guide

This guide walks through InfraWerf screen by screen. Menu and button labels are shown as they appear in the app. If a label has changed in your version, look for the same action nearby.

- [First steps](#first-steps)
- [The map](#the-map)
- [Server workspace](#server-workspace)
- [Inventory](#inventory)
- [Health](#health)
- [Files and transfers](#files-and-transfers)
- [Terminal](#terminal)
- [Services and containers](#services-and-containers)
- [Apps](#apps)
- [Settings](#settings)
- [Locking and sessions](#locking-and-sessions)
- [Working with saved data](#working-with-saved-data)

Before you start, skim the [security model](security.md). It explains why some actions ask you to authenticate again.

## First steps

1. **Open InfraWerf.** The map appears. On a new install it is empty.
2. **Add a server.** Create an entry on the map and set its **connection hostname**: the name or alias you already use for it in your OpenSSH configuration (for example, a `Host` entry in `%USERPROFILE%\.ssh\config`). Use the alias, not an IP, if your config uses one.
3. **Verify SSH access.** Open the server, go to **Settings → Verify SSH access**.
   - InfraWerf resolves the alias with your system OpenSSH configuration and shows the endpoint and the server's **fingerprint**.
   - Compare that fingerprint with one you trust, such as the one you saw when you first connected from a terminal.
   - Only continue if they match. For a new or changed fingerprint, verify it through your normal SSH process outside InfraWerf. InfraWerf does not change your known-hosts file.
4. **Refresh inventory** (see [Inventory](#inventory)) to save what the server reports about itself.

A verification is a short check. It does not open a protected session. Files, Terminal, transfers and service actions each need their own authentication.

> **Limits:** direct aliases work. Jump hosts, proxy commands and known-host helper configurations are not supported yet; such servers show as unavailable for verification.

## The map

The map shows your servers as cards and the relationships between them.

- **Add** a server from the map's toolbar or menu.
- **Move** cards by dragging. Positions are saved as you change them.
- **Rename** a server, or change its connection hostname, from its card or its Settings tab.
- **Link** two servers to record that one depends on or relates to another. Choose the link kind and its line style if the relationship has one.
- **Pan and zoom** with the mouse wheel and drag, or from the keyboard. Use the keyboard shortcuts listed in the map's help to move between cards.

Cards show what InfraWerf last saw, with a timestamp. A card can be **healthy**, **unhealthy**, **stale** (the last observation is old) or **unknown** (InfraWerf has not checked). **Stale is not offline**: it means the saved information is old, not that the server is down.

Map edits are saved to your local database immediately. Removing a server from InfraWerf changes only InfraWerf's saved copy. It never changes the server itself.

## Server workspace

Open a server to reach its workspace. It has these tabs:

| Tab | Purpose |
| --- | --- |
| **Services** | Saved services and containers from the last inventory. |
| **Health** | TCP and HTTP checks and their results. |
| **Files** | Protected file browser (SFTP). |
| **Terminal** | Interactive shell. |
| **Apps** | Web applications on the server's localhost, opened through a private tunnel. |
| **Plugin data** | Results from commands that plugins run on this server. |
| **System** | Saved system inventory: host details, services and containers from the latest complete collection. |
| **Settings** | Name, connection hostname and Verify SSH access. |
| **Transfers** | The queue of uploads and downloads for this server. |

Plugins can add tabs and sections to this workspace. They appear beside the built-in ones.

## Inventory

Inventory is what InfraWerf has saved about a server. It appears in the **System** and **Services** tabs and in the **Inventory** view on the map.

- **Refresh** asks the server, over an authenticated SSH session, for its current inventory. The result is saved with the time it was received.
- If a refresh only partly succeeds, InfraWerf keeps the earlier data and shows what failed. It does not silently throw away good data.
- **Search** filters the saved list.

Inventory is saved information. It tells you what was there when it was last refreshed, not what is there now.

## Health

The **Health** tab runs two checks against a server, separately:

- **Host**: whether a TCP port accepts connections.
- **Service**: an HTTP GET to a port and path, within a time limit.

Enter the host TCP port, the service port and path, and the check interval, then choose **Save & start checks**. **Stop checks** ends them. Checks only go to the addresses you entered and do not log in. Results stay in memory.

Each result is one of:

| State | Meaning |
| --- | --- |
| **Reachable** or **healthy** | The latest check succeeded. |
| **Unhealthy** | The latest check failed. |
| **Stale** | The last result is older than the freshness limit. It says nothing about whether the server is up now. |
| **Unknown** | Nothing has been checked yet. |

Each result shows when it was last checked and when it last succeeded.

## Files and transfers

Files opens a **protected** SFTP session. Opening it asks you to authenticate.

With a file session open you can:

- **Browse** folders and see file details.
- **New folder**, **Rename**, **Move** and **Delete**. Each change shows an exact-path preview and needs your confirmation.
- Use the SSH account's own permissions. InfraWerf cannot do more than that account can.

Safety rules that apply here:

- **No overwrite** and **no recursive delete** from the Files tab. To replace a file, delete or rename the old one first.
- **Move** locks the source while you choose a destination, so the file cannot change under you.
- If a change's outcome is uncertain (for example, the connection drops after you confirm), InfraWerf closes the session and asks you to inspect the server before trying again.
- The session ends after **five minutes idle** or **thirty minutes** in total. Leaving Files, hiding InfraWerf, or pressing **Lock** clears what is shown.

**Transfers** moves files between your PC and a server:

- Choose a local file to upload, or a remote file to download, using the normal Windows file picker.
- Each transfer streams with progress, can be cancelled, and is checked with SHA-256 at the end.
- Downloads never overwrite an existing file on your PC.
- Transfers use short-lived permissions that expire after ten minutes.
- Closing InfraWerf during a transfer warns you first.

## Terminal

Terminal opens an interactive shell on the server. It uses the same authenticated approach as Files.

- Input and output are shown live, with Unicode support. The window resizes with the panel.
- **Ctrl+C** sends an interrupt to the running program, as it would in a normal terminal.
- Output is capped at 64 KiB of history in the panel. Older output scrolls away.
- The terminal contents are never saved to your database.
- **Lock** or disconnecting clears the view and ends the session.

## Services and containers

The **Services** tab shows what runs on the server and how you can control it:

- It reads **systemd** units (system and user), **Podman** containers (user) and **Docker** containers, when those are present on the server.
- Live state comes from an authenticated session. Each item shows its state, or says why it cannot tell: *stopped*, *denied*, *unavailable*.
- **Logs** show the most recent lines, capped at 8 KiB and cut safely at character boundaries.
- **Controls** (start, stop, restart) are scoped: InfraWerf only offers the actions the server's permissions and your configuration allow.

Every control needs a **fresh authentication**. Each action is reviewed, asks you to confirm, and gives a clear result. If the result is uncertain, the item is marked and you should check it on the server before acting again.

## Apps

Apps lists web applications that run on a server's own localhost (for example, a dashboard on port 8080 that is not exposed to your network).

- Opening an app creates a **private tunnel** from your PC to that localhost port, opened in its own window.
- The tunnel is loopback-only on your PC and closes when you close the window, press Lock, or the session ends.
- InfraWerf does not expose the app to your network.

Apps are declared by plugins (see [plugins](plugins.md)). Only the port and path an app declares are reachable.

## Settings

There are two levels.

- **Application Settings**: appearance (glass or classic, light or dark), plugins, and the owner PIN.
- **Server Settings** (in each server's workspace): name, connection hostname and Verify SSH access.

Changes are saved when you confirm them.

### Owner PIN

You can set a PIN to stop other people using the app. It guards changes to settings and plugin permissions. A PIN does not encrypt your data; anyone with access to your Windows account can still read the data folder. Use Windows account protection for that.

## Plugins

Plugins are installed and reviewed in **Settings → Plugins**. See the [plugins guide](plugins.md) for how they are installed, what each permission means, and how to remove one.

## Locking and sessions

**Lock**:

- closes every protected session (Files, Terminal, transfers, Apps tunnels, workload control),
- clears the results shown from them,
- requires authentication again before any of those return.

Lock before you step away, before you quit, or when you hand the PC to someone else. Lock before quitting as well: quitting ends the sessions, but Lock makes the clearing explicit.

## Working with saved data

- Everything you add (servers, map layout, relationships, saved inventory and settings) is stored in one local SQLite database. See [Data and backup](data-and-backup.md).
- Saved data is never a live view. Look at the timestamp before you act on it.
- Deleting something in InfraWerf removes InfraWerf's copy only.
