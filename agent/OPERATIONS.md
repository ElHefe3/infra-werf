# Managed operation guard foundation (T032)

The shared `atlas_core::operations` module guards typed operations. T033 supplies connection verification; T014/T015 integrate protected desktop sessions and actual read-only SFTP. Phone pairing/coordinator, sudo and mutations remain unavailable. There is no generic executor or grant endpoint.

## Trusted inputs and lifecycle

`Authority` cannot be serialized/deserialized or printed through Debug. A trusted backend adapter issues it only after validating its native window or paired application session. It registers a scoped SSH session **after actual SSH authentication**, never because a caller supplied a DTO or ID. Remote expiry/revocation must revoke the same broker authority. T014's FileSessions supplies desktop SSH idle/absolute lifetime and protected-view clearing; T013 retains fresh per-action sudo and T011 supplies paired app-session policy.

The adapter takes `ServerTarget` from the current Rust store before both prepare and execute, and checks it again before returning results. Caller/session/server/host mismatches deny access. System OpenSSH resolves and pins hostname/user/port while preserving explicit HostKeyAlias semantics; strict trust verification binds the actual connection. Helpers/proxies remain unsupported. Workload IDs and canonical roots originate in authenticated discovery, not frontend claims.

The backend prepares an opaque single-use ticket; execute accepts its ID and a fresh backend target, with no replacement command or arguments. Read-only SystemSummary, FileMetadata, ServiceStatus and SftpRead plans exist; only SFTP is exposed as protected native execution in this slice. SftpRead resources are backend-issued and registered to the authenticated principal/session/host. Unknown fields, shell, terminal, sudo and mutation requests fail closed. Future coordinator adapters must use bounded decoding and fixed redacted parse failures.

ConnectionCheck and SftpOpen bootstrap plans are backend-only, cannot be requested through InspectionRequest and cannot substitute for each other or protected tickets. `account_output` consumes captured verification/config bytes without exposing diagnostics as a result. Settings uses a fixed nonce and ends its connection. Files requires genuine publickey success plus an SFTP v3 handshake, then registers a protected session. Neither invents an authenticated identity before verification.

## Execution, bounds and cleanup

Maximum budgets: 30 seconds per ticket, 64 KiB UTF-8 output, four actual executor workers, 128 active authorities/sessions, 1,024 retained prepared/current tickets and 200 audit entries. Backend tests may lower time/output budgets. Time enforcement uses monotonic Instant; displayed expiries are informational UTC timestamps.

Each ticket starts once. Failure/cancellation/expiry/revocation exposes no partial output; returned executor errors are reduced to known error codes/fixed messages. Audit contains operation/server IDs and an optional error code, never payload, paths, credentials or raw diagnostics. Successful `ProtectedText` is transient and has no automatic route to SQLite or baseline inventory.

The broker returns at its deadline even if an executor ignores cancellation. Such a worker remains counted until it actually exits; it cannot emit an accepted late result. This does **not** kill OS children. Real SSH/SFTP adapters must check the execution context while reading, terminate/close child processes/channels on cancel, and retain T008's native operation lease inside the executor for its entire lifetime. Do not release the native close guard merely because a timed-out caller returned. Trusted executors must not log/panic with raw protected payloads; the library does not install or alter global panic/log hooks.

## Remote paths

FileMetadata's lexical/canonical helpers do not prove filesystem confinement and have no native executor. Actual Files uses bounded SFTP v3 packets, raw path bytes and opaque resource IDs; labels never reconstruct paths. The scope is the SSH account's filesystem, not a claimed home jail. Home is obtained through REALPATH after authentication; root browsing is explicit. LSTAT preserves symlink identity and following a directory link is explicit. Future restricted-root policies/mutations require separate race/confinement proof.

Each service belongs to one backend principal. Sessions idle after five minutes and expire absolutely after 30; lowered test policies prove both independently. They retain one OpenSSH child and the native admission lease through actual kill/wait/pipe-thread joins. Lock, authority revocation, server edit/removal, timeout/disconnect, leaving Files and document hiding invalidate/clear results. Every close queues a bounded metadata notification; overflow requests clearing all protected views. No session automatically resumes.

Listings contain at most 100 entries and fit the broker output budget; wire frames are capped at 1 MiB, paths/names at 4 KiB, resource maps at 512 and live desktop sessions at eight. Cursor tokens are single-use and old page-entry IDs expire. A denied candidate listing preserves the prior page/handle. Sizes are decimal strings to retain full u64 precision. Results are ephemeral, with no Store route. No file content or write opcode exists.

## Verification

`mise run test:feature -- T032` runs controlled broker tests. T033 uses `test:ssh` and `test:ssh-native`. T014/T015 reuse workspace checks with `test:feature -- T014`, actual SFTP lifecycle proof with `test:files`, and native browse/stat/lock/error proof with `test:files-native`. No real keys/trust/infrastructure are test targets. Paired clients, sudo, writes, transfers and physical-phone acceptance remain their tickets.
