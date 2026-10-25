# Managed operation guard foundation (T032)

The shared `atlas_core::operations` module is a **library foundation**, not SSH authentication, a generic executor API or a production transport. No new Tauri/coordinator operation endpoint exists. T012/T014 must integrate actual authentication/execution; phone pairing, sudo and mutations remain unavailable.

## Trusted inputs and lifecycle

`Authority` cannot be serialized/deserialized or printed through Debug. A trusted backend adapter issues it only after validating its native window or paired application session. It registers a scoped SSH session **after actual SSH authentication**, never because a caller supplied a DTO or ID. Remote expiry/revocation must revoke the same broker authority. T013 supplies SSH idle/absolute lifetime and protected-view clearing; T011 supplies app-session policy.

The adapter takes `ServerTarget` from the current Rust store before both prepare and execute. Caller/session/server/host mismatches deny access. Actual SSH resolution must also bind the resolved endpoint and trusted key in T012; this foundation has no SSH config resolver. Workload IDs and canonical roots originate in authenticated discovery, not frontend claims.

The backend prepares an opaque single-use ticket; execute accepts its ID and a fresh backend target, with no replacement command or arguments. Only read-only SystemSummary, FileMetadata and ServiceStatus plans exist. Unknown fields, shell, terminal, sudo and mutation requests fail closed. Native/coordinator adapters should use `InspectionRequest::decode` for a 32 KiB wire limit and fixed redacted parse failures.

## Execution, bounds and cleanup

Maximum budgets: 30 seconds per ticket, 64 KiB UTF-8 output, four actual executor workers, 128 active authorities/sessions, 1,024 retained prepared/current tickets and 200 audit entries. Backend tests may lower time/output budgets. Time enforcement uses monotonic Instant; displayed expiries are informational UTC timestamps.

Each ticket starts once. Failure/cancellation/expiry/revocation exposes no partial output; returned executor errors are reduced to known error codes/fixed messages. Audit contains operation/server IDs and an optional error code, never payload, paths, credentials or raw diagnostics. Successful `ProtectedText` is transient and has no automatic route to SQLite or baseline inventory.

The broker returns at its deadline even if an executor ignores cancellation. Such a worker remains counted until it actually exits; it cannot emit an accepted late result. This does **not** kill OS children. Real SSH/SFTP adapters must check the execution context while reading, terminate/close child processes/channels on cancel, and retain T008's native operation lease inside the executor for its entire lifetime. Do not release the native close guard merely because a timed-out caller returned. Trusted executors must not log/panic with raw protected payloads; the library does not install or alter global panic/log hooks.

## Remote paths

FileMetadata plans retain literal POSIX paths, not shell strings. Component-aware lexical containment rejects traversal and sibling-prefix escapes. `ScopedPath::check_resolved` checks server-canonical path containment for an adapter. This helper does not prove filesystem confinement: F08 must handle symlink/rename races, obtain canonical roots and test actual SFTP behavior before exposing operations. No managed path is executed by this foundation.

## Verification

`mise run test:feature -- T032` runs focused broker tests plus generated contract/type/tracker checks. Tests use private in-memory grants and fake executors, including hostile requests, competing callers, replay, cancellation/revocation, sticky output failure, deadline return and worker saturation/recovery. No keys, known-host files or real infrastructure are read or modified. Actual SSH/SFTP/native/phone journeys remain their existing tickets.
