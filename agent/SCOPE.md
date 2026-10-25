# Atlas product scope

## Goal and owner decisions

Atlas is a personal infrastructure map and authenticated server workspace on the owner's Windows PC and native Android phone. Preserve the approved light desktop and compact phone designs while replacing the provisional Python/vanilla-JavaScript architecture.

Owner decisions, 25 October 2026: **core release first; native Android; Windows PC first; LAN/existing VPN; full core phone controls; latest stable tools through mise.** Two usage windows are an efficiency target, not permission to omit security, data preservation, packaging or acceptance.

## Core release

| Capability | Required behavior |
| --- | --- |
| Native clients | Windows x64 Tauri application and signed sideloadable Android ARM64 APK; shared typed React components with distinct compositions |
| Map/inventory | React Flow editing, directional relationships, saved layout, searchable hosts/workloads, collection timestamps and honest stale/unknown state |
| Phone access | Authenticated Windows coordinator over LAN/existing VPN with full core controls; no public endpoint or new VPN |
| Workspaces | Files, Services, Terminal, System, Settings and Transfers, usable on 13-inch laptops and small phones |
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
6. Use allowlisted operations and safe argument boundaries; service/container identities come from authenticated discovery. Arbitrary terminal input is confined to the user's explicitly authenticated interactive session.
7. Protected uploads stage under the SSH user, preview destination/ownership changes, then elevate only application. Retain safe recovery information for partial transfers without credentials.
8. Existing infrastructure reports are read-only. Tests must not incidentally change real files, workloads, trust, grants, firewall/router settings or production services.

## Deferred and excluded

- S07: custom agents, enrolment/modules, always-on server deployment and privileged-collection exceptions.
- S08: rsync and its Windows runtime; later copy/update default, dry-run and explicit destination deletion.
- No iOS/macOS/Linux packages, public endpoint, new VPN, multi-user roles, app-store publication or cloud sync.
- No automatic network/proxy/deployment rewiring from diagram edges, infrastructure migrations, broad root/container grants, floating-window desktop, remote-desktop streaming or visual redesign.

## Core acceptance

- [ ] Pinned mise tools build installable Windows and native Android artifacts, with actual phone interaction verified.
- [ ] IDs/names/layout/relationships/approved inventories survive migration and restart; tested rollback preserves the reference and original data until acceptance.
- [ ] Both clients share domain data and retain approved styling/usability.
- [ ] Pair/revoke, failed/changed host trust, expiry/reconnect and protected-result clearing pass controlled tests.
- [ ] Protected inspection/fresh sudo rules hold without secret or protected-result leakage to persistent public state/logs.
- [ ] File browse/edit/upload/download, interruption recovery, terminal and scoped service journeys pass on both clients.
- [ ] Reachability and service health stay distinct; failed checks and old observations remain truthful.
- [ ] Every core ticket and owner/device acceptance has evidence; unresolved release blockers remain open.

These are requirements, not current implementation claims. [AGENT.md](AGENT.md) defines sequencing and status authority.
