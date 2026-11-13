# D052 — Owner direction: InfraWerf as an open-source plugin platform

Date: 2026-11-11. Related: S12 / F18–F25 / T037–T051.

This records the owner's direction from a planning conversation. It does **not** activate S12; the scope is drafted as deferred and its tickets may not start until the owner activates them.

## What the owner decided

- **Direction:** move InfraWerf towards plugins/add-ons so its functionality can be "expanded drastically".
- **Vision:** an open-source platform other people install and extend. "Treat this as open source software, nobody will be adding addons to my instance": each owner alone decides what runs on their own instance. No multi-user roles, shared instances or remote installs.
- **Plugin kinds wanted:** service integrations; monitoring and data; actions and automation; new views and tools.
- **Audience now:** only the owner; design decisions serve the owner's lab first.
- **Plugin power:** trusted automation. A plugin the owner explicitly trusts may act without asking each time.

## Working assumptions (not confirmed by the owner)

The owner did not answer these four questions; the draft assumes:

1. Plugins exist to keep the core small and let a community build integrations.
2. Jobs run while the app is open at first; a background or server runtime is a later decision.
3. Authoring is mixed: data files for simple plugins, TypeScript for views, WebAssembly for logic.
4. The first real plugin serves the owner's lab (assumed: a backup overview).

## Decisions still needed at activation

- Settled by [D055](D055-plugin-choices-belong-to-the-owner-in-settings.md): how far a plugin may act, and whether trusted automation may use elevation, are each owner's choice per plugin and per server, with safe defaults. Recording the matching change to SCOPE.md rule 6 is still needed before F21.
- The default trust level for a newly enabled plugin.
- Licence, and whether the repository becomes public (currently a private Forgejo remote).
- Whether the future Android client (S09) supports plugins.
