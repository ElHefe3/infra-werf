# D055 — Plugin choices belong to each owner, in one Settings screen

Date: 2026-11-13. Related: S12 / F18 T052, F20 T041–T042, F22.

## Owner decisions

1. **The trust questions are each owner's choice, not fixed in the code.** Asked how the two open decisions from [D052](D052-plugin-platform-direction.md) should be settled (how far a plugin may act, and whether trusted automation may use sudo), the owner chose to leave them to each instance owner, as options in the application.
2. **One place for everything that is not about a single server.** The owner said: "I would like it all together (any non server specific settings)". InfraWerf gets one application-wide Settings screen. Settings about one server stay in that server's Settings tab.

3. **No default trust level: ask each time.** Asked whether a newly enabled plugin should start as read-only, the owner chose "Ask each time" on 2026-11-13. Enabling a plugin always requires an explicit choice of trust level; nothing is preselected and a plugin cannot be enabled without one.

4. **One configuration page per plugin, holding all its permissions.** The owner asked on 2026-11-13: "Can we have a config screen for a plugin that contains all the permissions?" Yes. Each plugin has one page in the application Settings screen that is the single place to see and change everything it may do.

## What this means

- **Per-plugin grants, chosen explicitly.** A plugin declares what it wants; the owner picks a trust level when enabling it (read-only, changes with confirmation, or trusted automation), per server. No level is preselected (decision 3). Elevation is a separate, off-by-default exception for one plugin, one server and one action. There is no blanket "may use sudo" option.
- **Still fixed for every instance,** because they protect owners who install plugins they did not write: no plugin gets SSH keys, sudo passwords or a raw SSH session; a plugin cannot use what it did not declare; an update that asks for more needs a new approval; a plugin's commands are shown before it is enabled.
- **Grants are stored in the native database and enforced in the Rust core.** The screen displays and edits them; it is not where they are enforced.

## Where each setting lives

| Place | Contents |
| --- | --- |
| Application Settings screen (new, T052) | Everything not tied to one server: the list of installed plugins, the control that turns all plugins off, and the existing application-wide options (history retention, SSH alias discovery). |
| A plugin's configuration page, inside that screen (T041) | Everything about one plugin: what it declared (capabilities and, for packs, the exact commands), enable and disable, its trust level, which servers it may act on, any elevation exception per server, its own settings, and its recent activity. This is the only place its permissions are changed. |
| A server's Settings tab (exists) | The existing map details, SSH connection and elevation check, plus a read-only list of the plugins allowed on this server that links to each plugin's page. |

## Still open

- The remaining working assumptions in D052.
