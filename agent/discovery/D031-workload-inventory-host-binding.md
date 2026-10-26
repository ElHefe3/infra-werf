# D031 — Live workload identities must remain bound to their authenticated SSH identity

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T020, T022

## Finding

The saved baseline was attached to a stable Atlas server ID, while the owner can edit the SSH entry's endpoint or account. Matching only the alias can therefore issue a saved workload identity to a different host or user session, including a different rootless Podman or user-systemd scope.

## Evidence

Code review on 2026-10-26 found that `Store::update_server` preserves the baseline when `host` changes, and that alias equality alone does not detect changes to resolved hostname, user, port, or authenticated key. T020's `refresh_inventory` already receives only a complete observation returned by a host-pinned authenticated SSH session.

## Consequence

`BaselineInventory.source_host` records the Atlas SSH alias only after a complete authenticated refresh. `source_binding` is a non-secret digest over the resolved SSH profile (alias, hostname, user, port, and identity-file count) and the verified server-key fingerprint. `FileSessions` issues workload IDs only when both the alias and binding match its authenticated session. Imports and old observations deserialize without a binding, remain available as saved information, and cannot issue live workload identities. Each issued ID also binds Atlas server ID, kind, runtime scope, runtime, and workload name. Unsupported runtime/scope combinations have no command fallback.

An equal-time complete observation may update the SSH binding only when every classified inventory field matches after provenance fields are removed. A different workload list at the same timestamp stays unbound. Older observations do not rebind.

## Follow-up

T022 tests imported/unbound and profile-mismatched identities, equal-time snapshot differences, wrong-host session use, and supported adapter resolution. Preserve the full authenticated-session check in later workload-control work; never derive a command from frontend-provided runtime, scope, name, or shell text. See [D033](D033-workload-identity-binds-ssh-profile.md) for the alias/account/key review finding.
