# Atlas product scope

## Goal and owner decisions

Atlas is a personal infrastructure map and authenticated server workspace on the owner's Windows PC. Native Android is future development (S09). Preserve the approved light desktop and compact phone designs while replacing the provisional Python/vanilla-JavaScript architecture.

Owner decisions, 25 October 2026: **core release first; native Android; Windows PC first; LAN/existing VPN; full core phone controls; latest stable tools through mise.** Two usage windows are an efficiency target, not permission to omit security, data preservation, packaging or acceptance.

## Current desktop release

The owner authorised the dynamic desktop extension on 2026-10-27: [S10](scopes/S10-live-desktop.md) / [F16](features/F16-live-desktop.md). InfraWerf checks opted-in servers and refreshes approved inventories while the desktop app is open, records bounded change history, discovers candidates from literal primary OpenSSH aliases, and supports archive/ignore/restore/local deletion. Owner-reported offline excludes contact. Always-on collectors and mobile remain future development.

Owner decision, 2026-10-26: **get desktop 100%; mobile is future development**. All remaining desktop batches are authorised. Preserve mobile requirements in [S09](scopes/S09-future-mobile.md); do not treat them as desktop release gates.

## Desktop capabilities

| Capability | Required behavior |
| --- | --- |
| Native clients | Windows x64 Tauri application; compact Android composition/APK retained in future S09 |
| Map/inventory | React Flow editing, directional relationships, saved layout, searchable hosts/workloads, collection timestamps and honest stale/unknown state |
| Phone access | Deferred to S09: authenticated Windows coordinator over LAN/existing VPN, with no public endpoint/new VPN |
| Workspaces | Files, Services, Terminal, System, Settings and Transfers, usable on 13-inch laptops; small-phone composition is future S09 |
| Health | Independent host reachability/service health, last check/success/failure and freshness; no invented utilisation |
| SSH | Existing system OpenSSH configuration/local agent, explicit host trust and protected session lifecycle |
| Files | SFTP navigation/stat/permissions, mkdir, rename/move and explicitly confirmed deletion |
| Transfers | Streaming native upload/download, queue/progress/cancel/retry, reviewed protected-destination staging |
| Operations | Authenticated terminal, service/container inspection/logs and reviewed scoped start/stop/restart with elevation |
| Recovery | Preserved SQLite IDs/layout/relationships, verified migration and rollback, install/restart instructions |

## Architecture and interface boundaries

- React/TypeScript/React Flow; Tauri on Windows and Android; Rust domain/persistence/operations; SQLite; system OpenSSH/agent and SFTP. Resolve latest stable compatible versions at setup, then pin through project mise and dependency lockfiles.
- The Rust domain layer is independent of the native shell. Desktop commands and the coordinator API invoke the same operations. Android adapters handle native file URIs and protected credential storage; never expose the PC shell or its private keys to the phone frontend.
- Define shared versioned contracts once: stable IDs, observation source/time/freshness, session/operation handles, bounded progress and structured errors. Preserve safe map semantics, but do not expose the legacy unauthenticated HTTP mutation surface on the LAN.
- Pair over TLS with explicit certificate trust/pinning, single-use short-lived secret, visible desktop approval and revocable device identity. Store credentials using platform-protected storage. No unauthenticated shared listener, universal TLS-validation bypass or production use of a development server.
- The Windows app owns coordinator lifecycle and shows availability. Avoid silently killing active phone/transfer work on ordinary window close. Initial release requires the PC awake and Atlas running.

## Authentication, persistence and operation rules

1. Overview retains only approved baseline inventory/health. Classify imported metadata before reuse.
2. Protected files/logs/processes/configuration are fetched on demand after SSH authentication. Pre-collection plus a UI lock is insufficient.
3. Keys remain outside frontend/database/evidence/logs; retain identity references only. No incidental key creation/copying.
4. Each privileged operation explicitly requests sudo and identifies server/action/path. Never save/log passwords, reuse them for unrelated work, alter global sudo policy or grant a permanent bypass.
5. Lock/expiry/revocation closes protected sessions, invalidates handles and clears results. Interrupted work cannot silently resume under expired credentials.
6. Use allowlisted operations and safe argument boundaries; service/container identities come from authenticated discovery. Arbitrary terminal input is confined to the user's explicitly authenticated interactive session. Exception (D057, 2026-11-16): a read-only command declared by a plugin pack may run once the owner has reviewed its exact arguments, enabled the plugin and allowed it on that server; it is an argument list with no shell and no sudo, takes values only from authenticated discovery, and runs under the built-in output and time limits.
7. Protected uploads stage under the SSH user, preview destination/ownership changes, then elevate only application. Retain safe recovery information for partial transfers without credentials.
8. Existing infrastructure reports are read-only. Tests must not incidentally change real files, workloads, trust, grants, firewall/router settings or production services.

## Deferred and excluded

- S07: custom agents, enrolment/modules, always-on server deployment and privileged-collection exceptions.
- S08: rsync and its Windows runtime; later copy/update default, dry-run and explicit destination deletion.
- S11 (activated 2026-11-08, D051): remote applications — loopback-only SSH local forwards to a server's `localhost` web interfaces in an isolated in-app browser window, then optionally reviewed graphical applications. Narrowly widens the no-forwarding SSH boundary for owner-started loopback forwards only.
- S12 (activated 2026-11-12, D053; direction in D052): extensibility — plugins and add-ons on a small core, with capability grants enforced in the Rust core, owner consent, an activity log and sandboxed code tiers. From F21 it widens rule 6 to read-only commands declared by plugins the owner has reviewed and enabled (owner decision D057, 2026-11-16); commands that change a server, sudo and scheduled runs are not covered by that decision. No plugin may obtain keys, sudo passwords or a raw SSH session. Plugins are installed only from a folder, file or `https://` address the owner chooses, fetched only when the owner presses Install (D058).
- No iOS/macOS/Linux packages, public endpoint, new VPN, multi-user roles, app-store publication or cloud sync.
- No automatic network/proxy/deployment rewiring from diagram edges, infrastructure migrations, broad root/container grants, floating-window desktop, remote-desktop streaming or visual redesign. Exception (D056, 2026-11-14): detachable copies of main-menu categories that float inside the single application window.

## Desktop release acceptance

- [ ] Pinned mise tools build an installable Windows release with actual native interaction verified.
- [ ] IDs/names/layout/relationships/approved inventories survive migration and restart; tested rollback preserves the reference and original data until acceptance.
- [ ] Desktop retains approved styling/usability and shared domain contracts for future clients.
- [ ] Failed/changed host trust, lock, expiry/reconnect and protected-result clearing pass desktop controlled tests; pairing/device revoke remains S09.
- [ ] Protected inspection/fresh sudo rules hold without secret or protected-result leakage to persistent public state/logs.
- [ ] File browse/edit/upload/download, interruption recovery, terminal and scoped service journeys pass on Windows.
- [ ] Reachability and service health stay distinct; failed checks and old observations remain truthful.
- [ ] Every desktop core ticket and owner acceptance has evidence; unresolved release blockers remain open.

These are requirements, not current implementation claims. [AGENT.md](AGENT.md) defines sequencing and status authority.
