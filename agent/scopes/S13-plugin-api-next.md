# S13 — Deferred plugin API, next version

<!-- atlas-scope: {"id":"S13","features":["F26"],"depends_on":["S12"],"window":null,"deferred":true} -->

## Outcome

The plugin API gaps found while building the reference plugin (T051) are closed in the next plugin API version, without widening what a plugin may do on a server.

## Entry and sequence

The owner activates S13 after S12 is complete and published. Each ticket raises the plugin API version or is a fix within it; [the compatibility rule](../../docs/plugin-api/README.md#compatibility-rule) decides which.

Order is by value to plugin authors: patterns and per-workload results first, then composed parameters, then findings and overviews, then clearer errors.

**Scope dependencies:** [S12](S12-extensibility.md). **Budget:** deferred; outside the core target.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F26](../features/F26-plugin-api-gaps.md) | T054–T058 | A plugin author writes the backup overview they actually wanted: every backup on every server, with its next run and its user-scope workers, in one place. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] The reference plugin uses each closed gap, and its workarounds are removed.
- [ ] No closed gap lets a pack run something it could not before: no shell, no sudo, no program outside the program rule, no value the owner did not choose or discovery did not find.
- [ ] Plugins written for the previous API version still load and behave as before.

## Verification and refinement

Manifest tests for every new rule; native suites for what the owner sees; the reference plugin's own suite.

## Exclusions and stop boundary

Changing commands, sudo, network access or anything D057, D061 or the program rule refuse. New gaps found while closing these become their own tickets.

## Exit

The reference plugin, rewritten with the new API version, shows every backup across servers.
