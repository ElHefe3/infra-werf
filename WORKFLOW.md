# Atlas working context

Use **Atlas implementation** as the active working chat on both Android Remote and the Windows desktop. It is the same local Codex thread, ID `01a171dd-e913-7541-a0e1-e515e7b7aea2`, in the local Random GPT project. On 25 October 2026 the desktop navigation tool opened this exact ID and the owner confirmed the same recent messages were visible on the PC.

The earlier **Build Infrastructure Map** conversation is a ChatGPT Work conversation, ID `6add26e2-c1e8-83ea-8e53-361dec360bcc`, in the linked ChatGPT Random GPT project. The desktop inventory lists both histories. Equal project names do not make the two conversations or history stores identical. Official documentation: https://learn.chatgpt.com/docs/projects and https://learn.chatgpt.com/docs/remote-connections.

The supported chat reader exposed only five recent exchanges from the earlier conversation, including after following its pagination cursor. The owner then supplied the complete available conversation as `../archive/infrawerf-origin-chat.md`, which has now been read. Its earlier stack discussion is recorded in STACK.md. The owner also confirmed the original chat opens on the phone using its direct ChatGPT link; the missing sidebar listing itself has not been repaired.

## Decisions to preserve

- The current tracker is [agent/AGENT.md](agent/AGENT.md), entered through [.agent.md](.agent.md). Its [scope](agent/SCOPE.md) supersedes older open phone/runtime assumptions: native Android, PC first, full phone controls, LAN/existing VPN, latest stable tools through mise, agents/rsync deferred. Use the active feature packet rather than rereading all chat history.

- Keep the approved Atlas desktop appearance and the separate compact phone design.
- A server workspace contains Services, System, Files, Terminal, Settings and Transfers. File operations need authenticated SSH; privileged actions need deliberate sudo.
- Support SSH-only hosts and optional restricted discovery agents, with an optional always-on coordinator.
- **The recovered original stack is React + TypeScript + React Flow, Tauri, Rust, SQLite and system OpenSSH/local SSH agent, with SFTP first and optional rsync later.** See STACK.md for the exact responsibilities, access requirements and unresolved choices. The current Python/SQLite/vanilla-JavaScript build is provisional and needs migration; preserve its data and approved designs.

## Current code and data

- Repository: this `atlas` directory. Local coordinator and separate desktop/mobile views; see README.md and IMPLEMENTATION.md.
- Persistent data: ignored `data/atlas.sqlite3`. Preserve it during any migration.
- Four saved host reports, 540 workload records. These are historical observations, not live monitoring.
- Host process can be started with `Start-Atlas.ps1 -Background`; local port 8770. Authenticated phone network access, SSH, Files, Terminal and agents are pending.
- Existing infrastructure-maintenance project is `../ops`; its reports are read-only inputs for this app.

## Continuity

Record confirmed product and stack decisions in repository documents when they change. Keep actual implementation status separate from the intended architecture. This lets either interface resume the same project even when a sidebar or conversation-reading API omits history.
