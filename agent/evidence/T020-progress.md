# T020 — Approved SSH inventory refresh progress

Date: 2026-10-26<br>
State: completed; superseded by [T020 acceptance](T020-acceptance.md)

## Outcome and boundaries

Collect only the fields already approved in [INVENTORY.md](../INVENTORY.md) through a deliberate, authenticated desktop SSH refresh. Keep host/workload identity stable, preserve owner map edits and relationship records, and retain the source collection timestamp. Keep unapproved raw output and protected details out of SQLite, UI results, and logs. No sudo, installed agent, runtime grant, automatic refresh loop, Android UI, or real owner host is part of this ticket.

## Proof plan

- Focused Rust tests for the remote-output parser, partial/malicious reports, timestamp ordering, preservation of IDs/names/host/layout/edges, and no update on authentication/collection failure.
- Actual authenticated SSH against the existing disposable local OpenSSH fixture; use synthetic baseline fields only.
- Native Windows UI journey to refresh, display updated/partial/failure states, and verify the saved map remains unchanged except for approved inventory and its event.
- Bounded independent review of the authenticated operation and persistence boundary.

## Investigation history

T005 validates the approved baseline and discards unknown fields. T013 provides authenticated OpenSSH/SFTP sessions; `Peer` pins a verified host key and supports bounded command execution inside the session lifecycle. The implementation uses a user-triggered operation and a fixed command allowlist; incomplete results leave the last approved snapshot intact and report partial status. Final evidence is recorded in [T020 acceptance](T020-acceptance.md).

## Stop condition

Do not add inventory fields, privileged commands, root/container grants, background collection, or runtime configuration changes. Pause this ticket if the existing allowlist cannot be collected without a new permission or if source time cannot be represented honestly. No real server will be probed.
