# Backup overview — example plugin

The example built step by step in the [plugin author guide](../../docs/plugin-api/guide.md). It declares a badge on each server card and a Backups tab in a server's workspace.

This example declares plugin API v1, which defines the manifest only, so this folder holds just `infrawerf-plugin.json`. For a plugin that runs commands, see [`host-basics`](../host-basics). The automated tests validate it, register it in the extension registry and compare it with the guide, so it cannot drift from the API.

Check it:

```
mise run exec -- cargo build -p atlas-core --bin infrawerf-plugin
target\debug\infrawerf-plugin.exe check examples\backup-overview
```
