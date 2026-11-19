# Plugins

Plugins add to InfraWerf without changing its code. A plugin can add a server badge, a tab in a server's workspace, a menu entry, a relationship style, a remote app entry, a read-only command on a server, or its own page and sandboxed job. A plugin is a folder that contains one manifest file, `infrawerf-plugin.json`, and sometimes a few supporting files.

This page is for **owners**: installing, reviewing and removing plugins. Authors will find the details in [For authors](#for-authors).

## What a plugin can and cannot do

- A plugin **asks** for permissions. It gets nothing until you allow it.
- Each permission (for example *read saved state*, *run an approved read-only command*, *read files*, *control workloads*) can be granted per server.
- A plugin never receives your SSH keys or a raw SSH session.
- A plugin's own page and jobs run in isolated sandboxes.
- Installing a plugin never runs any of its scripts.

## Install a plugin

Open **Settings → Plugins → Install a plugin**. Choose one of three sources:

| Source | What it is |
| --- | --- |
| **A folder** on this PC | Copied into your plugins folder. The original stays where it is. |
| **An archive** (`.zip` or `.tar.gz`) | Unpacked into the plugins folder. |
| **An `https://` address** | Downloaded once, when you press Install, then unpacked. Only `https://` is accepted. |

Before anything is written, InfraWerf checks the size, paths, file types and the manifest. A refused install leaves nothing behind.

A newly installed plugin is **disabled**. Open its page and review it before you enable it.

### Check the hash

Each install records two SHA-256 hashes, shown on the plugin's page:

- **Archive SHA-256**: the archive or download as you received it.
- **Files SHA-256**: the plugin's files.

If the author published an expected hash, enter it before you install. InfraWerf refuses an install whose hash differs.

### Signatures

Some authors sign their archives with [minisign](https://jedisct1.github.io/minisign/). If a signature is present, InfraWerf can check it against the author's public key. A valid signature shows who signed the archive; it does not make the plugin safe.

## Review a plugin

On the plugin's page, check:

1. **Requested permissions**: what the plugin asks to do, and for which server types.
2. **Contributions**: which tabs, badges, menu items or commands it adds.
3. **Hashes and signature** status.
4. **Author and licence**, as the manifest states them.

Enable only what you need. You can enable a plugin and still leave a particular permission off for a particular server.

## Changed files

InfraWerf hashes each installed plugin's files at every start. If a plugin's files change, gain a file or lose one, it shows **Files changed since it was installed**, and its permissions are blocked until you remove it and install it again from a copy you trust.

A plugin you copied into the plugins folder by hand has no recorded hash. InfraWerf still checks its manifest.

## Remove a plugin

On its page, choose **Remove**. This deletes the plugin's folder, its permissions and its install record. A command still running for it is cancelled, and its app tunnels are closed. The plugin's activity history remains in the activity log.

## Plugin trust levels

Trust is per plugin, and you set it. Plugins you have not reviewed should stay at the lowest level. Raise it only for plugins you have read.

## For authors

Start with the [plugin author guide](plugin-api/guide.md). It takes you from an empty folder to a manifest that InfraWerf accepts, using the [backup overview example](../examples/backup-overview).

| Topic | Page |
| --- | --- |
| Every manifest field | [Manifest reference](plugin-api/README.md) |
| Read-only commands, extractors, workload kinds, badges, applications, relationship kinds | [Packs](plugin-api/packs.md) |
| A plugin's own page | [Views](plugin-api/views.md) |
| Sandboxed jobs in WebAssembly | [Plugin logic](plugin-api/logic.md) |
| Archives, hashes and signatures | [Sharing and installing](plugin-api/install.md) |
| What stays stable between versions | [Compatibility](COMPATIBILITY.md) |

Working examples are in [`examples/`](../examples):

| Example | Shows |
| --- | --- |
| `backup-overview` | A server badge and a workspace tab, manifest only. |
| `host-basics` | Read-only commands on a server. |
| `service-summary` | A sandboxed job. |
| `systemd-backups` | Commands, a health check, a badge and a job working together. |
| `host-dashboard` | A workspace tab that shows the plugin's own view. |
| `homelab-map` | Relationship kinds, a remote app, a badge and a command. |

Check a manifest before you share it:

```powershell
mise run exec -- cargo build -p atlas-core --bin infrawerf-plugin
target\debug\infrawerf-plugin.exe check path\to\plugin
```
