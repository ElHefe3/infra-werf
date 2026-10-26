# F04 — Native Windows desktop shell and lifecycle over the shared Rust/React domain.

<!-- atlas-feature: {"id":"F04","scope":"S02","depends_on":["F03","F01"],"deferred":false} -->

Current owner decision (2026-10-26): desktop is the complete release target. Phone/coordinator/APK requirements are preserved in [S09](../scopes/S09-future-mobile.md), separately deferred. All desktop batches are authorised sequentially.

## Work packet

**Outcome:** A real Windows desktop client with shared domain behavior and a guarded native lifecycle. Android composition, adapters and phone access remain future S09 work.

**User journey:** Launch the Windows client → open the local map/workspace → handle lifecycle without losing active work.

**Scope:** [S02](../scopes/S02-applications-map.md). **Feature prerequisites:** [F03](F03-react-map.md), [F01](F01-build-environment.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D002](../discovery/D002-rust-host.md), [D003](../discovery/D003-android-prerequisites.md), [D014](../discovery/D014-desktop-guard-phasing.md), [D015](../discovery/D015-operation-decoding-and-admission.md), [D026](../discovery/D026-active-session-lifecycle-count.md). Shared guard API/integration obligations: [OPERATIONS.md](../OPERATIONS.md).

## T008 — Integrate Windows Tauri and coordinator lifecycle

<!-- atlas-ticket: {"id":"T008","feature":"F04","status":"done","depends_on":["T002","T003","T004","T007"],"evidence":"../evidence/F04-desktop-acceptance.md"} -->

### Outcome and included work

Run the Rust application/domain in the Windows Tauri shell with explicit local coordinator lifecycle.

### Exclusions

Always-on server deployment, automatic startup service installation or public network exposure.

### Implementation and contracts

Replace dependency on the Python foreground/background launcher in the new build. Restrict native capabilities; route commands through the same domain operations. Show coordinator availability, handle single-instance/close/quit behavior and guard active operations. Desktop-first staging: the remote listener stays unavailable until F05. F05/F08/F09 must register remote sessions/transfers with the shared close guard before enabling them; actual remote-work close/warning journeys belong to those features.

**Ticket dependencies:** [T002](F01-build-environment.md), [T003](F02-domain-persistence.md), [T004](F02-domain-persistence.md), [T007](F03-react-map.md).

### Acceptance criteria

- [x] Native Windows app loads real persisted data and invokes restricted Rust domain commands.
- [x] Local workspace startup/restart errors and remote-coordinator availability are visible; native close refuses active domain work. Desktop transfer close-warning proof is recorded in T017; remote phone/coordinator registration remains future S09 work.
- [x] The legacy database/reference remains recoverable; no unauthenticated LAN binding or frontend arbitrary shell access is introduced.

### Verification

Native launch/restart and lifecycle smoke journey plus targeted capability/error checks; no full installer cycle yet.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T008, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **passed for T008** in [Windows acceptance](../evidence/F04-desktop-acceptance.md); phone/coordinator proof remains in future S09. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record platform/lifecycle blockers; do not paper over failures with the Python app while claiming Rust completion.

## T032 — Build the desktop managed-operation guard

<!-- atlas-ticket: {"id":"T032","feature":"F04","status":"done","depends_on":["T003","T008"],"evidence":"../evidence/T032-acceptance.md"} -->

### Outcome and included work

Prepare a shared backend guard for the desktop's later SSH operations without waiting for the compact phone/pairing work. This is a split of T014's foundation, not an added product capability or a claim that T014 is complete.

### Exclusions

Live host access, SSH authentication, pairing, credential onboarding, native protected-operation IPC, terminal/sudo/mutations and actual SFTP filesystem confinement. Those remain their existing tickets.

### Implementation and contracts

Use backend-only, non-deserializable authority/session grants; bind prepared read-only operations to caller, session and a backend server snapshot. Reject unknown operation/authority fields. Validate scoped paths and authenticated workload IDs; pass typed plans rather than shell command strings. Enforce bounded output, deadline, concurrency and single-use tickets. Cancel/revoke/expiry discard unsuccessful payloads; keep safe bounded audit metadata only. Actual SSH/SFTP adapters must cancel/terminate child work, validate canonical remote paths and join the native lifecycle guard when integrated in T012/T014/F08.

**Ticket dependencies:** [T003](F02-domain-persistence.md), [T008](F04-native-shells.md).

### Acceptance criteria

- [x] Controlled backend authority/session grants enforce caller, host, session, revocation and expiry boundaries; no frontend DTO can grant authority.
- [x] Only typed read-only inspections are accepted; malformed/extra shell, credential, mutation, path and workload requests are denied or handled as literal scoped data without executing shell strings.
- [x] Deadline/output/concurrency limits, cancellation and single-use execution fail predictably; failed results expose no partial payload or raw executor errors, and audit contains only bounded safe metadata.

### Verification

Focused Rust tests using controlled executors, including an uncooperative read-only worker, concurrent/replay cases and synthetic secret markers. Shared contract/regression and portable-core compile as appropriate. Obtain a bounded independent review. No native UI rebuild is required for a library-only foundation.

### Evidence and closure

[T032 acceptance](../evidence/T032-acceptance.md) records passing controlled guard tests, generated/Windows/portable-core checks and resolved independent review. Actual SSH/phone/mutation integration remains open work; tracker validation rules are unchanged.

### Stop conditions

Do not expose a generic executor or authority-grant endpoint to Tauri/coordinator callers. Stop any live adapter integration until actual trust/authentication, cancellation and output-retention proof passes.

## T033 — Verify desktop SSH access with strict host trust

<!-- atlas-ticket: {"id":"T033","feature":"F04","status":"done","depends_on":["T008","T032"],"evidence":"../evidence/T033-acceptance.md"} -->

### Outcome and included work

In native desktop server settings, resolve the selected concrete OpenSSH alias and perform one real, bounded public-key authentication check. This extracts desktop connection verification from T012/T014; paired transport and protected-session/operation acceptance stay there.

### Exclusions

Password/key onboarding, automatic trust acceptance/known-host writes, jump/proxy/helper configuration, persistent SSH sessions, sudo, file/terminal/service controls and live owner infrastructure changes.

### Implementation and contracts

Use the system OpenSSH binary and existing configuration/agent/key references. A backend-only, single-use connection-check ticket uses the T032 authority/resource/cancellation guard without inventing an authenticated SSH session. Execute only a fixed echo nonce with strict host checking and no forwarding/multiplexing/backgrounding/helper/trust updates. Show resolved host/user/port, safe host fingerprint and distinct unknown/changed/authentication/network/config/client outcomes. Never return raw stdout/stderr, key contents or private-key paths. Close/cancel/timeout must terminate/reap the owned child and retain the native lifecycle lease until cleanup. Refuse unsupported proxy/jump/helper configurations explicitly.

**Ticket dependencies:** [T008](F04-native-shells.md), [T032](F04-native-shells.md).

### Acceptance criteria

- [x] Native settings perform a real strict-trust OpenSSH verification using configured public-key identity references, with honest transient results and no persistent SSH-session claim.
- [x] Controlled trusted/unknown/changed/rejected-auth/unreachable cases are distinct; fingerprints require independent verification and no known-host/config contents are changed by Atlas.
- [x] Missing client, unsupported config, output overflow, cancellation and timeout fail safely; native close protects active work, no raw diagnostics/credentials reach UI/SQLite, and no generic shell/grant endpoint is added.

### Verification

Isolated loopback SSH server and synthetic fixture-only keys/config/trust files, real system OpenSSH, focused broker/process tests and one actual native WebView2 journey. Independent bounded review. Reuse unaffected map acceptance; no Android UI/APK rebuild.

### Evidence and closure

[T033 acceptance](../evidence/T033-acceptance.md) records passing real-process/native proof, data/trust preservation, resolved independent review and explicit limits. [D016](../discovery/D016-real-openssh-verification.md) records learned OpenSSH and cleanup constraints.

### Stop conditions

Never auto-accept unknown/changed real keys or use a trust bypass to finish. Only the explicitly controlled fixture may generate temporary test identities. Actual user trust/credential changes require their applicable authorization.
