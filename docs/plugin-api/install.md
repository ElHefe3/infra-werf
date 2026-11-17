# Sharing and installing plugins

InfraWerf installs a plugin from one of three places the owner chooses in **Settings → Plugins → Install a plugin**. There is no catalogue and nothing is installed automatically.

| From | What happens |
| --- | --- |
| A folder on this PC | Its files are copied into the plugins folder. The original is left as it is. |
| An archive file | A `.zip` or `.tar.gz` file is unpacked into the plugins folder. The kind is read from the file's content, not its name. |
| An `https://` address | That one address is fetched once, when the owner presses Install, then unpacked as above. Only `https://` is accepted, and a redirect to `http://` is refused. |

A plugin that installs arrives **disabled**: installing allows nothing. The owner reviews and enables it on its page, as for any plugin. No install script is run, ever: a plugin is data.

## What is checked

Everything is read and checked in memory before anything is written. A refusal leaves nothing in the plugins folder.

- **Size.** An archive or download is at most 5 MiB. A plugin's files are at most 20 MiB in all, 5 MiB each, and 256 files. A path is at most 200 characters and 8 folders deep. These limits apply to what the archive really holds, not to what its headers claim.
- **Paths.** Every path stays inside the plugin's folder: no absolute paths, drives, `..`, `.`, empty names, Windows device names such as `CON`, names ending in a dot or space, or the characters `< > : " | ? *`. Two files whose names differ only in upper and lower case are refused.
- **Entries.** Only plain files and folders. Symbolic links, hard links, devices and any other special entry are refused, from an archive or a folder.
- **Shape.** `infrawerf-plugin.json` is at the top of the archive, or in exactly one folder at the top (as an archive made from a folder is). That folder level is dropped when installing.
- **Manifest.** It must be valid, exactly as for a plugin placed by hand.
- **Not already installed.** A plugin with the same ID must be removed first. Updating a plugin in place comes later.

The plugin is then written to a working folder inside the plugins folder, read back and compared with what was checked, and moved into place with one rename as `plugins/<plugin id>`.

## Hashes

InfraWerf records two SHA-256 hashes and shows both on the plugin's page:

- **Archive SHA-256**: of the archive file or download, exactly as received.
- **Files SHA-256**: of the plugin's files. For each file in path order: its path, its size and its own SHA-256. Folders are not part of it.

The owner may enter an **expected SHA-256** before installing: of the archive for an archive or an address, of the plugin's files for a folder. If it differs, the install is refused.

**At every start-up** InfraWerf computes the files hash of each plugin it installed and compares it with the recorded one. A plugin whose files changed, gained a file or lost one is shown as "Files changed since it was installed" and nothing it asks for is allowed until the owner removes it and installs it again. A folder placed in the plugins folder by hand has no recorded hash; its manifest is still checked for changes as before.

## Signatures

An author may sign an archive with [minisign](https://jedisct1.github.io/minisign/):

```text
minisign -Sm my-plugin.zip
```

This writes `my-plugin.zip.minisig`. Publish it next to the archive, at the same address with `.minisig` added, and publish your public key (the line starting with `RW` in `minisign.pub`).

InfraWerf trusts no key by itself. The owner pastes the author's public key into the install form when they want the signature checked. Then a missing signature, a signature by another key, or an archive changed after signing refuses the install. A verified signature's key ID is shown on the plugin's page. Without a key, the signature is not checked and the page says so. A folder cannot be signed.

## Removing

**Remove** on a plugin's page deletes its folder from the plugins folder and forgets its trust level, its permissions on every server and its install record. A pack command in flight is cancelled and its application tunnels are closed. The plugin activity log keeps its history. A folder that is not a usable plugin can be removed the same way.

## Error codes

| Code | Meaning |
| --- | --- |
| `unreadable` | The source could not be read or fetched. |
| `not_https` | The address is not a plain `https://` address. |
| `unsupported_archive` | Neither a zip nor a gzip-compressed tar file. |
| `path_traversal` | A path would leave the plugin's folder, or is not a safe plain name. |
| `unsafe_entry` | A link, device or other special entry. |
| `too_large` | Over a size, count or depth limit. |
| `no_manifest` | No manifest at the top or in a single top folder. |
| `invalid_manifest` | The manifest is not valid; its own codes follow. |
| `hash_mismatch` | Not the SHA-256 the owner expected. |
| `signature_missing` | A public key was given but there is no signature. |
| `signature_invalid` | The signature does not match the archive and key. |
| `invalid_public_key` | Not a minisign public key. |
| `signature_not_possible` | A public key was given for a folder. |
| `already_installed` | A plugin with this ID, or a folder with its name, is already there. |
