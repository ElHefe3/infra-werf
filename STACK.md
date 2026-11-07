# Atlas technology stack and requirements

Recovered 25 October 2026 from the owner-supplied [full conversation dump](../archive/infrawerf-origin-chat.md), under **What would be the tech stack?** and the subsequent file-transfer discussion. This records the original stack; it supersedes the provisional implementation choices in A01.

## Original desktop stack

| Layer | Required technology | Responsibility |
| --- | --- | --- |
| Interface | React + TypeScript | Server cards, inspectors, workspaces and authentication prompts |
| Interactive map | React Flow | Draggable/custom/nested nodes, meaningful edges, zoom and saved layout |
| Desktop application | Tauri | Native desktop package and restricted access to local functionality |
| Backend | Rust | Health checks, SSH sessions, operations and access enforcement |
| Storage | SQLite | Inventory, relationships, layouts and approved status history |
| Server access | System OpenSSH client + local SSH agent | Existing SSH configuration and identities; host-key verification |
| Initial monitoring | HTTP/TCP checks + periodic SSH commands | Separate reachability/service checks; authenticated protected inspection |
| File management | SFTP over SSH first | Browse, upload, download, rename and move |
| Directory synchronisation | Optional rsync over SSH later | Efficient/resumable directory transfers; Windows runtime choice still needed |

The transcript explicitly recommended a local Tauri/Rust desktop application, with agentless access initially. Later discussion extended that to optional restricted discovery agents and an optional always-on coordinator. Those extensions do not replace the original desktop stack.

## Product and access requirements

- Preserve the approved light Atlas styling, the 13-inch desktop layout and the separate compact phone interface approved in the implementation chat.
- The map shows servers and their contents, meaningful directional relationships, reachability, service health, collection time and stale observations. A reachable host does not imply healthy applications.
- Drawing a relationship initially documents intent. Applying network/proxy/deployment changes is a separate explicit operation.
- A server opens an inspector and a compact workspace with Files, Services, Terminal, System, Settings and Transfers. Prefer one main app, tabs and optional split view on small displays.
- Overview shows only approved baseline inventory and health. Protected files, logs, processes and configuration must be collected on demand through a short-lived authenticated SSH session. Hiding previously collected protected data behind a UI prompt does not satisfy the requirement.
- Keep private keys outside the frontend and SQLite; store identity references only. Use the local agent and verify host identity.
- Privileged operations need an explicit sudo prompt identifying the server, action and affected path. Do not save or log sudo passwords. A fresh-password requirement needs deliberate handling of the remote sudo timestamp policy.
- Lock/expiry closes protected access and clears protected results according to policy; do not retain privileged results by default.
- Transfers share SSH authentication. Copy/update is the rsync default; destination deletion is explicit and preceded by a change preview.
- Protected uploads stage under the SSH user, show the destination changes, then elevate to apply ownership/permissions.
- Optional agents collect approved baseline modules under a restricted account, enrol with a single-use token, receive revocable device identities and do not provide a permanent privileged shortcut.

## Reconciliation with A01

| Requirement | Current implementation | Required next work |
| --- | --- | --- |
| React + TypeScript | Vanilla JavaScript | Port approved interfaces to typed React components |
| React Flow | Handwritten map/gestures | Replace map mechanics with React Flow while preserving appearance and layout |
| Tauri | Browser page plus Python launcher | Add the actual desktop application |
| Rust backend | Python HTTP coordinator | Move persistence/domain operations into Rust; define a restricted desktop command boundary |
| SQLite | Implemented | Preserve server IDs, layouts, relationships and database contents; verify migration/parity |
| Shared phone interface | Local browser route implemented | Keep separate phone design; choose/authenticate its connection to the coordinator |
| Live checks | Saved snapshots only | Implement reachability and per-service health without fabricating live status |
| SSH / SFTP / sudo | Not implemented | Build authenticated sessions, host verification and expiry before protected operations |
| rsync / agents | Not implemented | Later increments after SSH/SFTP and authentication proof |

The Python application remains a provisional working increment and parity reference. Preserve its database and keep it available until the replacement is verified. Saved inventories are historical observations; review which fields qualify for approved baseline retention before carrying them into the final access model.

## Subsequent owner decisions

The tracker-planning discussion selected a **native Android Tauri app**, **Windows PC first**, **LAN/existing VPN**, and **full core phone controls**. The core excludes custom agents, always-on server deployment and rsync, which remain follow-up scopes. [agent/SCOPE.md](agent/SCOPE.md) and [agent/AGENT.md](agent/AGENT.md) record the acceptance boundary and implementation order.

Use latest stable Node.js Current, Rust, pnpm and compatible dependencies, resolved once and pinned in project-local mise configuration and package/Cargo lockfiles by T001. Select the Windows MSVC host per project; Android JDK/SDK/NDK must meet the current stable Tauri/Gradle compatibility requirements. Record exceptions; do not silently use old PATH tools or change unrelated global defaults. The current Python app remains the parity reference until verified cutover.

## Still open for deferred work

- Always-on coordinator deployment host and its network/credential policy; the core host is already the Windows PC.
- Agent implementation language/protocol and privileged module exceptions.
- Windows rsync distribution/runtime.

No framework substitutions or new permanent privileges are implied by these remaining choices. Exact latest-stable version pins are an execution-time T001 deliverable, not historical planning observations.
