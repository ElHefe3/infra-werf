# D055 — Plugin choices belong to each owner, in one Settings screen

Date: 2026-11-13. Related: S12 / F18 T052, F20 T041–T042, F22.

## Owner decisions

1. **The trust questions are each owner's choice, not fixed in the code.** Asked how the two open decisions from [D052](D052-plugin-platform-direction.md) should be settled (how far a plugin may act, and whether trusted automation may use sudo), the owner chose to leave them to each instance owner, as options in the application.
2. **One place for everything that is not about a single server.** The owner said: "I would like it all together (any non server specific settings)". InfraWerf gets one application-wide Settings screen. Settings about one server stay in that server's Settings tab.

## What this means

- **Per-plugin grants with safe defaults.** A plugin declares what it wants; the owner picks a trust level when enabling it (read-only, changes with confirmation, or trusted automation), per server. Elevation is a separate, off-by-default exception for one plugin, one server and one action. There is no blanket "may use sudo" option.
- **Still fixed for every instance,** because they protect owners who install plugins they did not write: no plugin gets SSH keys, sudo passwords or a raw SSH session; a plugin cannot use what it did not declare; an update that asks for more needs a new approval; a plugin's commands are shown before it is enabled.
- **Grants are stored in the native database and enforced in the Rust core.** The screen displays and edits them; it is not where they are enforced.

## Where each setting lives

| Place | Contents |
| --- | --- |
| Application Settings screen (new, T052) | Everything not tied to one server: installed plugins, enable and disable, consent and trust level, the plugin activity log, the control that turns all plugins off, and the existing application-wide options (history retention, SSH alias discovery). |
| A server's Settings tab (exists) | Which plugins may act on this server and any elevation exception for it, beside the existing map details, SSH connection and elevation check. |

## Still open

- The default trust level for a newly enabled plugin. Proposed: read-only. This default applies to every instance, so it is the project owner's call.
- The remaining working assumptions in D052.
