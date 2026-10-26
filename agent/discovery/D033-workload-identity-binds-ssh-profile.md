# D033 — Workload identity provenance must bind the resolved SSH profile

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T022; preserve for T023

## Finding

An SSH alias is user-editable indirection. The same alias can later resolve to another host, port, or account; a host key alone also does not distinguish accounts on that server. Reusing inventory IDs based only on the alias can inspect a same-named service or container in the wrong user scope.

## Evidence

Independent T022 review identified the alias-only gap. The final implementation stores a non-secret digest of the resolved SSH alias, hostname, user, port, identity-file count, and verified host-key fingerprint with a complete authenticated inventory. A newly opened session issues workload IDs only when its resolved profile and key reproduce that digest. The Windows native proof changed the fixture account while retaining the alias and host key; it received no old workload IDs and the UI requested a fresh inventory.

## Consequence

The digest is a consistency binding, not an authentication token. Imports and older inventory cannot supply it. Equal-time rebinding also requires exact equality of the complete classified snapshot after provenance fields are removed.

## Follow-up

Use the same server, profile, account, and key binding for workload control in T023. Keep any mismatch fail-closed and tell the user to refresh the approved inventory.
