# D026 — Active authenticated sessions hold the native lifecycle lease

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T008, T013, T014

## Finding

`workspace_status.pending_operations` reports the count of active native lifecycle leases. An authenticated SFTP session deliberately retains one lease for as long as its SSH child/session remains open. The count is therefore expected to be `1` during that session, including while an elevation preview is cancelled or a sudo attempt completes. Lock/close must release it and return the count to `0`.

## Evidence

On 2026-10-26, the Windows native elevation journey initially failed because its test expected zero immediately after cancelling a preview. `Workspace::open_files` passes its admission lease into `FileSessions::open`; the SSH client stores it with its child in `crates/atlas-core/src/sftp.rs`. After correcting the expectation to one while open and zero after Lock, the full native journey passed. The app's close-admission behavior is also specified in `src-tauri/src/workspace.rs`.

## Consequence

Treat this status field as the lifecycle/close-admission count, not as a count of pending one-shot commands. Native tests for protected sessions must assert the lease remains held until Lock, expiry or cleanup. The observed count was not an operation leak.

## Follow-up

Reuse this contract for T021–T023 and release lifecycle proof. If UI needs a one-shot pending-command count later, expose a separate metric rather than changing the close guard's meaning.
