# Atlas implementation

| ID | Increment | State | Acceptance |
| --- | --- | --- | --- |
| M01 | Align A01 with the recovered original technology stack | Next | React/TypeScript + React Flow interfaces, Tauri desktop, Rust domain/persistence layer; preserve SQLite IDs/data/layout and prove parity before replacing the Python reference |
| A01 | Local coordinator, database, shared desktop/mobile map and saved inventory | Done | Persistent servers/positions/connections; truthful observations; API and browser checks recorded in IMPLEMENTATION.md |
| A02 | Authenticated phone access and coordinator deployment | Planned | Choose host, authenticate owner, expire sessions, verify access on actual phone |
| A03 | SSH sessions and host identity verification | Planned | Verify host keys; use existing local identities; no saved passwords; disconnect/expiry proof |
| A04 | Files and transfers | Planned | Browse/download/upload over authenticated SSH; explicit destinations; protected-path elevation; transfer progress |
| A05 | Continuous discovery agents | Planned | Approved modules, single-use enrolment, revocable device identities and stale/offline handling |
| A06 | Terminal and service operations | Planned | Interactive authenticated session; scoped operations and audit records; deliberate sudo |

The approved visual references are the desktop Atlas prototype and the separate phone design. STACK.md records the recovered original stack and requirements. A01 was completed using a provisional Python/vanilla-JavaScript stack; M01 corrects that deviation before further architecture-dependent implementation. This application has its own repository and database; infrastructure migration, existing inventory grants and homelab workloads remain outside A01.
