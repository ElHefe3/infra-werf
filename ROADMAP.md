# Atlas implementation

| ID | Increment | State | Acceptance |
| --- | --- | --- | --- |
| A01 | Local coordinator, database, shared desktop/mobile map and saved inventory | Done | Persistent servers/positions/connections; truthful observations; API and browser checks recorded in IMPLEMENTATION.md |
| A02 | Authenticated phone access and coordinator deployment | Planned | Choose host, authenticate owner, expire sessions, verify access on actual phone |
| A03 | SSH sessions and host identity verification | Planned | Verify host keys; use existing local identities; no saved passwords; disconnect/expiry proof |
| A04 | Files and transfers | Planned | Browse/download/upload over authenticated SSH; explicit destinations; protected-path elevation; transfer progress |
| A05 | Continuous discovery agents | Planned | Approved modules, single-use enrolment, revocable device identities and stale/offline handling |
| A06 | Terminal and service operations | Planned | Interactive authenticated session; scoped operations and audit records; deliberate sudo |

The approved visual references are the desktop Atlas prototype and the separate phone design. This application has its own repository and database; infrastructure migration, existing inventory grants and homelab workloads remain outside A01.
