# Managed operation guard foundation (T032)

The shared `atlas_core::operations` module guards typed operations. T033 supplies connection verification; T014/T015 integrate protected sessions/browsing and T016 adds confirmed desktop SFTP mutations. Phone pairing/coordinator, content transfers, terminal and sudo remain unavailable. There is no generic executor or grant endpoint.

## Trusted inputs and lifecycle

`Authority` cannot be serialized/deserialized or printed through Debug. A trusted backend adapter issues it only after validating its native window or paired application session. It registers a scoped SSH session **after actual SSH authentication**, never because a caller supplied a DTO or ID. Remote expiry/revocation must revoke the same broker authority. T014's FileSessions supplies desktop SSH idle/absolute lifetime and protected-view clearing; T013 retains fresh per-action sudo and T011 supplies paired app-session policy.

The adapter takes `ServerTarget` from the current Rust store before both prepare and execute, and checks it again before returning results. Caller/session/server/host mismatches deny access. System OpenSSH resolves and pins hostname/user/port while preserving explicit HostKeyAlias semantics; strict trust verification binds the actual connection. Helpers/proxies remain unsupported. Workload IDs and canonical roots originate in authenticated discovery, not frontend claims.

The backend prepares an opaque single-use ticket; execute accepts its ID and a fresh backend target. Read-only SystemSummary, FileMetadata, ServiceStatus and SftpRead plans exist; protected native execution exposes SFTP. Confirmed SftpMutation plans use a separate dispatcher and cannot substitute for read/bootstrap tickets. Resources are backend-issued and bound to principal/session/host. Unknown fields, arbitrary paths/commands, shell, terminal and sudo requests fail closed. Future coordinator adapters must use bounded decoding and redacted parse failures.

ConnectionCheck and SftpOpen bootstrap plans are backend-only, cannot be requested through InspectionRequest and cannot substitute for each other or protected tickets. `account_output` consumes captured verification/config bytes without exposing diagnostics as a result. Settings uses a fixed nonce and ends its connection. Files requires genuine publickey success plus an SFTP v3 handshake, then registers a protected session. Neither invents an authenticated identity before verification.

## Execution, bounds and cleanup

Maximum budgets: 30 seconds per ticket, 64 KiB UTF-8 output, four actual executor workers, 128 active authorities/sessions, 1,024 retained prepared/current tickets and 200 audit entries. Backend tests may lower time/output budgets. Time enforcement uses monotonic Instant; displayed expiries are informational UTC timestamps.

Each ticket starts once. Failure/cancellation/expiry/revocation exposes no partial output; returned executor errors are reduced to known error codes/fixed messages. Audit contains operation/server IDs and an optional error code, never payload, paths, credentials or raw diagnostics. Successful `ProtectedText` is transient and has no automatic route to SQLite or baseline inventory.

The broker returns at its deadline even if an executor ignores cancellation. Such a worker remains counted until it actually exits; it cannot emit an accepted late result. This does **not** kill OS children. Real SSH/SFTP adapters must check the execution context while reading, terminate/close child processes/channels on cancel, and retain T008's native operation lease inside the executor for its entire lifetime. Do not release the native close guard merely because a timed-out caller returned. Trusted executors must not log/panic with raw protected payloads; the library does not install or alter global panic/log hooks.

## Remote paths

FileMetadata's lexical/canonical helpers do not prove filesystem confinement and have no native executor. Actual Files uses bounded SFTP v3 packets, raw bytes and opaque resources; labels never reconstruct paths. Literal backslashes are escaped distinctly from byte/control escapes. Scope is the SSH account's filesystem, not a home jail. Home uses REALPATH after authentication; root browsing is explicit. LSTAT preserves symlink identity; following a directory link is explicit. Restricted-root/atomic-inode policies require separate proof.

Each service belongs to one backend principal. Sessions idle after five minutes and expire absolutely after 30; lowered test policies prove both independently. They retain one OpenSSH child and the native admission lease through actual kill/wait/pipe-thread joins. Lock, authority revocation, server edit/removal, timeout/disconnect, leaving Files and document hiding invalidate/clear results. Every close queues a bounded metadata notification; overflow requests clearing all protected views. No session automatically resumes.

Listings contain at most 100 entries and fit the output budget; wire frames are capped at 1 MiB, paths/names at 4 KiB, resource maps at 512 and live sessions at eight. Cursors are single-use and old page entries expire. A denied candidate listing preserves the prior page/handle. Sizes are decimal strings for u64 precision. Results have no Store route. No file-content streaming exists yet.

## Confirmed file changes

Preview accepts only existing resource IDs plus a literal new-name component (1–255 UTF-8 bytes, no slash/control/dot traversal). One pinned source survives destination navigation. A single pending intent per session stores immutable backend paths, fresh metadata/type/canonical-parent checks and a maximum 30-second lifetime. The server/resolved endpoint and exact paths are displayed before separate confirmation. Cancel discards the intent. Browsing or replacing selection invalidates pending intent; tickets cannot be replayed or rebound to another host/session.

Commit rechecks source metadata/type, canonical parent directories and destination absence. It emits only MKDIR, standard non-overwrite RENAME, REMOVE or RMDIR. Links are removed as links; RMDIR cannot recurse. No overwrite extension, root/sudo fallback or recursive traversal exists. Standard SFTP/server filesystem fallbacks do not provide atomic inode comparison against concurrent writers; see [D019](discovery/D019-sftp-mutation-outcomes.md).

Valid acknowledgements are separate from transport errors. One atomic phase publishes not-sent/sent/rejected/applied. Lost delivery after a request may have applied returns OutcomeUnknown and closes the session; no automatic retry occurs. A parsed rejection returns a known redacted error; the client clears stale selection and refreshes actual directory state. Parent busy admission blocks tab/dialog exit through acknowledgement/reconciliation; explicit Lock remains available and preserves uncertainty. Protected lock/cancel precedes optional event teardown. Previews/receipts never persist in public inventory or audit payloads.

## Verification

`mise run test:feature -- T032` runs broker tests. T033 uses `test:ssh`/`test:ssh-native`. T014–T016 reuse workspace checks with `test:feature -- T016`, actual lifecycle/mutation proof with `test:files`, and combined native proof with `test:file-changes-native` (`test:files-native` runs browse-only). No real keys/trust/infrastructure are test targets. Paired clients, sudo, content transfers and physical-phone acceptance remain their tickets.
