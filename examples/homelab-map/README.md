# Homelab map — example pack

A plugin made only of data that adds to screens InfraWerf already has (plugin API v2). It is described in [packs](../../docs/plugin-api/packs.md#adding-to-existing-screens).

| Contribution | Adds |
| --- | --- |
| Socket units | A collector that lists systemd socket units, shown as workloads of the kind "Socket units". Runs `systemctl list-units --type=socket --all --no-legend --plain`. |
| System state | A health check that runs `systemctl is-system-running`. |
| System state (badge) | A badge on each server card with that health check's latest result. |
| Status page | `localhost:8099/status` on the server, in its Apps list. |
| Backs up to | A dashed blue relationship kind for the map. |

To try it, copy this folder into your plugins folder, open Settings → Plugins, review it and enable it. Allow it on a server to see its badge, its application and its relationship kind. The application needs "Confirm each change" or higher, because opening a tunnel is a change.
