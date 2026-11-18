# Host basics — example pack

A pack is a plugin made only of data. This one adds two collectors and two health checks, each a standard read-only command. It is the example used in [the pack reference](../../docs/plugin-api/packs.md).

| Contribution | Command | What is read |
| --- | --- | --- |
| Disk usage | `df -P -k` | A table, one row per file system. |
| Kernel version | `uname -r` | The first line. |
| System state | `systemctl is-system-running` | The first line; `running` means healthy. |
| Service is active | `systemctl is-active {unit}` | The first line; `active` means healthy. You pick the service from the server's inventory. |

To try it, copy this folder into your plugins folder (the path is shown in **Settings → Plugins**), enable it there, and allow both of its permissions on a server. Its commands then appear in that server's **Plugin data** tab.

Check it with:

```
target\debug\infrawerf-plugin.exe check examples\host-basics
```

The automated tests validate this file and compare it with the pack reference, so it cannot drift from the API.
