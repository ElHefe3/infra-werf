# S12 — Extensibility: plugins and add-ons

<!-- atlas-scope: {"id":"S12","features":["F18","F19","F20","F21","F22","F23","F24","F25"],"depends_on":["S06"],"window":2,"deferred":false} -->

## Outcome

InfraWerf becomes open-source software with a small core (map, SSH access, safety) and plugins for everything else: service integrations, monitoring and data, actions and automation, and new views. Each owner alone decides which plugins run on their own instance.

## Entry and sequence

The owner activated S12 on 2026-11-12 by starting T037 ([D053](../discovery/D053-owner-activates-extensibility.md)). Work stays on the `feature/s12-extensibility` branch and reaches `master` only when the whole feature set is done. From F21 onward the scope widens the "allowlisted operations" rule in [SCOPE.md](../SCOPE.md) to operations declared by plugins the owner has reviewed and enabled; that widening must be recorded as an owner decision before F21 starts; F18–F20 change no boundary. Direction and working assumptions are in [D052](../discovery/D052-plugin-platform-direction.md).

Deliver in this order: extension points (F18) → plugin API and manifest (F19) → permissions and consent (F20) → declarative packs (F21) → install and integrity (F22). That is the first public-ready increment. Sandboxed views (F23) and sandboxed logic with automation (F24) each need their own security review before their tickets start. Open-source readiness (F25) can proceed alongside once F19 exists; publication itself is an owner action.

The open desktop tickets (T034, T035) are still to be closed or re-scoped; F18 builds on the remote-applications implementation as shipped in 0.1.5.

**Scope dependencies:** [S06](S06-core-release.md). **Budget:** owner-activated extension after the core release.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F18](../features/F18-extension-points.md) | T037, T038, T052 | Open InfraWerf → every workspace tab, server-card badge, collector and operation is listed in a registry → remote applications still works, now registered as a built-in extension. |
| [F19](../features/F19-plugin-api-manifest.md) | T039, T040 | An author reads the guide → writes a manifest → runs the validator → sees exactly which rule failed or that the plugin is valid for API v1. |
| [F20](../features/F20-plugin-permissions.md) | T041, T042 | Install a plugin → review its requested capabilities per server → enable, optionally granting trusted automation → later open the activity log → disable the plugin with one action. |
| [F21](../features/F21-declarative-packs.md) | T043, T044 | Drop a pack into the plugins folder → review and enable it → new checks, collectors and badges appear for the granted servers. |
| [F22](../features/F22-plugin-install-integrity.md) | T045, T046 | Choose a plugin file or address → InfraWerf verifies it → review → enable; on update, any new capability is shown before the update applies. |
| [F23](../features/F23-plugin-views.md) | T047 | Enable a plugin with a view → its tab appears → it shows data and offers actions through the granted capabilities only. |
| [F24](../features/F24-plugin-logic-automation.md) | T048, T049 | Enable a trusted plugin → it runs a multi-step job or a schedule → every step is visible in the activity log and stops when trust is revoked. |
| [F25](../features/F25-open-source-readiness.md) | T050, T051 | A stranger finds the repository → installs InfraWerf → installs the reference plugin → reads the guide and writes their own. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] Built-in features are delivered through the same extension points plugins use.
- [ ] A plugin can do nothing its manifest did not request and the owner did not grant; enforcement is in the Rust core.
- [ ] No plugin can obtain private keys, sudo passwords, a raw SSH session or unrestricted local file access.
- [ ] The owner can see what each plugin did and disable any plugin immediately.
- [ ] The plugin API is versioned and documented, and a reference plugin built only on it runs on the owner's lab.
- [ ] Publication as open source is an explicit, recorded owner decision after a private-data audit.

## Verification and refinement

Controlled loopback SSH fixtures with fixture plugins, including hostile ones (unsafe commands, archive traversal, sandbox escape, capability overreach). Native Windows journeys for install, review, use, revoke and removal. No real-host testing without owner direction.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md).

## Exclusions and stop boundary

No native or unsandboxed script plugins, multi-user roles or shared instances, remote installation by anyone other than the owner, hosted plugin catalogue or store, server-side agents (S07) or always-on coordinator, and no jobs while the app is closed without a separate runtime decision.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work.

## Exit

A small core with a documented plugin API, a working first set of plugin tiers and a reference plugin; later tiers and publication remain separate owner decisions.
