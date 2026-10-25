# Atlas working context

Use **Atlas implementation** as the active working chat on both Android Remote and the Windows desktop. It is the same local Codex thread, ID `01a171dd-e913-7541-a0e1-e515e7b7aea2`, in the local Random GPT project. On 25 October 2026 the desktop navigation tool opened this exact ID and the owner confirmed the same recent messages were visible on the PC.

The earlier **Build Infrastructure Map** conversation is a ChatGPT Work conversation, ID `6add26e2-c1e8-83ea-8e53-361dec360bcc`, in the linked ChatGPT Random GPT project. The desktop inventory lists both histories. Equal project names do not make the two conversations or history stores identical. Official documentation: https://learn.chatgpt.com/docs/projects and https://learn.chatgpt.com/docs/remote-connections.

The supported chat reader currently exposes only five recent exchanges from the earlier conversation, including after following its pagination cursor. Those exchanges do not contain the earlier technology-stack discussion. Do not claim that the whole earlier conversation was read or that a missing passage proves a sync failure. Direct opening on the phone is a separate access test.

## Decisions to preserve

- Keep the approved Atlas desktop appearance and the separate compact phone design.
- A server workspace contains Services, System, Files, Terminal, Settings and Transfers. File operations need authenticated SSH; privileged actions need deliberate sudo.
- Support SSH-only hosts and optional restricted discovery agents, with an optional always-on coordinator.
- **The owner expects Tauri and says a technology stack was provided in the earlier chat. The remaining original stack has not yet been recovered.** The current Python/SQLite/vanilla-JavaScript build is a provisional implementation, not confirmation of the originally agreed stack. Recover and record that decision before continuing architecture-dependent implementation. Do not substitute a newly guessed stack.

## Current code and data

- Repository: this `atlas` directory. Local coordinator and separate desktop/mobile views; see README.md and IMPLEMENTATION.md.
- Persistent data: ignored `data/atlas.sqlite3`. Preserve it during any migration.
- Four saved host reports, 540 workload records. These are historical observations, not live monitoring.
- Host process can be started with `Start-Atlas.ps1 -Background`; local port 8770. Authenticated phone network access, SSH, Files, Terminal and agents are pending.
- Existing infrastructure-maintenance project is `../server map`; its reports are read-only inputs for this app.

## Continuity

Record confirmed product and stack decisions in repository documents when they change. Keep actual implementation status separate from the intended architecture. This lets either interface resume the same project even when a sidebar or conversation-reading API omits history.
