# D018 — SFTP transport and transactional page lifecycle

Date: 2026-10-26

State: verified

Affected: T014, T015

## Finding

The existing system OpenSSH client can carry a persistent, fixed SFTP subsystem without a shell or new credential store. SFTP v3 length-prefixed packets preserve raw path bytes; opaque backend resources keep display labels from becoming operation paths. Genuine public-key authentication and the subsystem handshake precede every broker session grant.

Windows `PeekNamedPipe` permits synchronous draining of SSH diagnostics already emitted before the SFTP VERSION response. A separate racing stderr consumer could otherwise miss a genuine `using none` diagnostic when deciding whether authentication succeeded. No raw diagnostics are sent to the UI or persisted.

The controlled AsyncSSH server may accept OPENDIR before READDIR evaluates permissions. Therefore successful OPENDIR alone cannot replace the current directory/page. The native journey exposed invalidated old handles after a denied listing; resources are now staged until the first page succeeds, and a denied candidate closes while the previous directory remains usable.

Review also exposed partial pipe-thread construction before child ownership and closures that did not notify an already visible file view. Child/lease ownership now starts before fallible pipe setup; both injected thread failures prove process termination, joins and lease release. Every close queues bounded metadata for the native view, including explicit backend lock and server invalidation; queue overflow requests clearing all protected views.

## Evidence

`mise run test:files` uses actual Windows OpenSSH and a private loopback fixture. It proves paging, single-use cursors, stale resources, lock/revoke, independently shortened idle/absolute limits, timeout and actual cleanup. Its regression attempts a denied directory then stats a retained-page file. Final native visual/invalidation and resolved independent review are recorded in [T014/T015 acceptance](../evidence/F06-F08-desktop-acceptance.md).

## Consequence

Use one persistent child per protected session, bounded binary frames and 100-entry/response-budget pages. Keep the native close lease inside the client until child and pipe-thread cleanup finishes. Default idle/absolute limits remain five/30 minutes; test policy may only shorten them. Lock/revoke denies delivery before cleanup. Read-only browsing covers the SSH account’s filesystem; it does not claim a home-directory jail or race-proof confinement.

## Follow-up

Reapply the same lifecycle to future paired adapters and mutations. Elevation, file content, writes and transfers remain separate tickets. A future restricted-root policy requires stronger confinement evidence than lexical or REALPATH checks alone.
