# Managed operation guard foundation (T032)

The shared `atlas_core::operations` module guards typed operations. T033 adds one backend-only connection-check bootstrap path and three purpose-specific native commands, with no authenticated SSH session grant. T012/T014 must still integrate protected/coordinator execution; phone pairing, sudo and mutations remain unavailable. There is no generic executor or grant endpoint.

## Trusted inputs and lifecycle

`Authority` cannot be serialized/deserialized or printed through Debug. A trusted backend adapter issues it only after validating its native window or paired application session. It registers a scoped SSH session **after actual SSH authentication**, never because a caller supplied a DTO or ID. Remote expiry/revocation must revoke the same broker authority. T013 supplies SSH idle/absolute lifetime and protected-view clearing; T011 supplies app-session policy.

The adapter takes `ServerTarget` from the current Rust store before both prepare and execute. Caller/session/server/host mismatches deny access. Actual SSH resolution must also bind the resolved endpoint and trusted key in T012; this foundation has no SSH config resolver. Workload IDs and canonical roots originate in authenticated discovery, not frontend claims.

The backend prepares an opaque single-use ticket; execute accepts its ID and a fresh backend target, with no replacement command or arguments. Only read-only SystemSummary, FileMetadata and ServiceStatus plans exist. Unknown fields, shell, terminal, sudo and mutation requests fail closed. Native/coordinator adapters should use `InspectionRequest::decode` for a 32 KiB wire limit and fixed redacted parse failures.

T033's ConnectionCheck plan is backend-only and cannot be requested through InspectionRequest or executed as protected access. Conversely, a protected ticket cannot be substituted for a bootstrap ticket. `account_output` consumes captured process bytes without exposing raw diagnostics as a result. Native settings resolve one selected alias and verify a fixed nonce, requiring genuine publickey success; none auth cannot verify an identity. No SSH session is invented before authentication.

## Execution, bounds and cleanup

Maximum budgets: 30 seconds per ticket, 64 KiB UTF-8 output, four actual executor workers, 128 active authorities/sessions, 1,024 retained prepared/current tickets and 200 audit entries. Backend tests may lower time/output budgets. Time enforcement uses monotonic Instant; displayed expiries are informational UTC timestamps.

Each ticket starts once. Failure/cancellation/expiry/revocation exposes no partial output; returned executor errors are reduced to known error codes/fixed messages. Audit contains operation/server IDs and an optional error code, never payload, paths, credentials or raw diagnostics. Successful `ProtectedText` is transient and has no automatic route to SQLite or baseline inventory.

The broker returns at its deadline even if an executor ignores cancellation. Such a worker remains counted until it actually exits; it cannot emit an accepted late result. This does **not** kill OS children. Real SSH/SFTP adapters must check the execution context while reading, terminate/close child processes/channels on cancel, and retain T008's native operation lease inside the executor for its entire lifetime. Do not release the native close guard merely because a timed-out caller returned. Trusted executors must not log/panic with raw protected payloads; the library does not install or alter global panic/log hooks.

## Remote paths

FileMetadata plans retain literal POSIX paths, not shell strings. Component-aware lexical containment rejects traversal and sibling-prefix escapes. `ScopedPath::check_resolved` checks server-canonical path containment for an adapter. This helper does not prove filesystem confinement: F08 must handle symlink/rename races, obtain canonical roots and test actual SFTP behavior before exposing operations. No managed path is executed by this foundation.

## Verification

`mise run test:feature -- T032` runs controlled broker tests. T033 adds real fixture-only OpenSSH/native proof via `test:ssh` and `test:ssh-native`, plus `test:feature -- T033`. No real keys/trust/infrastructure are test targets. Protected-session, actual SFTP and phone journeys remain their integration tickets; connection verification alone does not complete them.
