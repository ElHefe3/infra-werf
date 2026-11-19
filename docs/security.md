# Security model

This page describes what InfraWerf reads and changes on your PC and your servers, and its limits. To report a vulnerability, see [SECURITY.md](../SECURITY.md).

## How it works

- Data is stored on your PC. InfraWerf has no account system, cloud service or telemetry, and it connects only to servers you add.
- It uses your system OpenSSH and your existing public-key identities or SSH agent.
- A server's host key is checked and shown to you. It is not trusted automatically.
- Protected information, such as file listings, terminal output and logs, is fetched through an authenticated session when you open it. It is not collected beforehand and hidden in the interface.
- A change to a server shows what will happen and asks for confirmation. Privileged changes also ask for a password each time.

## What it does not do

- Copy, store or display private keys or passwords.
- Edit your known-hosts file, your SSH configuration, or the server's trust settings.
- Accept an unknown or changed fingerprint automatically.
- Overwrite a file in Files, or delete a directory recursively.
- Send your data, inventory or logs to anyone.
- Run an installer script from a plugin. Plugins are data, except for sandboxed jobs described below.

## SSH trust

Verification uses strict host trust:

- InfraWerf resolves the server's alias with your OpenSSH configuration and shows the endpoint and fingerprint.
- An unknown fingerprint must be checked through your own process, outside InfraWerf.
- A changed fingerprint is never accepted from inside the app. Resolve it the way you normally would, and then verify again.

Unsupported configurations (jump hosts, proxy commands, known-host helper programs) are refused, because InfraWerf cannot check trust through them.

## Protected sessions

Files, Terminal, transfers, Apps tunnels and workload control each require an authenticated session.

| Limit | Value |
| --- | --- |
| Idle timeout | 5 minutes |
| Absolute lifetime | 30 minutes |
| Transfer grants | 10 minutes |

Leaving the tab, hiding InfraWerf or pressing **Lock** closes sessions and clears what they showed. Privileged actions (`sudo`) need a fresh password for each action; a password is not kept.

A failed or uncertain change closes the session and asks you to inspect the server before trying again. InfraWerf does not guess that a change happened.

## Health checks

Health checks are the only checks that do not log in. They are TCP connections and HTTP requests to the addresses you configured, each with a time limit. They do not run commands on the server.

## Apps tunnels

An app is reached through a tunnel from a loopback port on your PC to a port on the server's own localhost. The tunnel accepts connections only from your PC and only while its window is open. Nothing is exposed to your network.

## Plugins

A plugin is a folder with a manifest. The manifest is a request, not a grant.

- A new plugin is **disabled** until you review it and enable it.
- Each capability (for example *run an approved read-only command*, *read files*, *control workloads*) is granted separately for each server.
- A plugin's files are hashed at install and at every start. If they change, the plugin's permissions are blocked until you reinstall it.
- Plugin logic runs in a **WebAssembly sandbox** with fixed host functions and limits.
- Plugin views run isolated and talk to InfraWerf only through a message API.

Read [plugins](plugins.md) before you enable one from someone else.

## Owner PIN

The PIN guards changes to settings and plugin permissions inside the app. It is not encryption. Protect the Windows account and its data folder ([data and backup](data-and-backup.md)).

## Code signing and integrity

The current installer is **not code-signed**. Its SHA-256 hash confirms the file is unchanged since it was produced. It does not identify the publisher. Verify the hash against the release notes, and only install builds you trust.

## Reporting a problem

Do not open a public issue for a security problem. Follow [SECURITY.md](../SECURITY.md).

## Limits to keep in mind

- InfraWerf is only as secure as your PC and your SSH setup. Keep Windows, OpenSSH and your keys protected.
- Saved data is readable by anyone who can read your Windows profile.
- Observations can be out of date. A healthy check from yesterday is not a statement about today.
