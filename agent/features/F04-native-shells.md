# F04 — Real native Windows and Android clients with a shared domain adapter and platform-specific lifecycle/I/O.

<!-- atlas-feature: {"id":"F04","scope":"S02","depends_on":["F03","F01"],"deferred":false} -->

## Work packet

**Outcome:** Real native Windows and Android clients with a shared domain adapter and platform-specific lifecycle/I/O.

**User journey:** Launch Windows coordinator → open both native clients → navigate the shared map/workspace → handle lifecycle without losing work.

**Scope:** [S02](../scopes/S02-applications-map.md). **Feature prerequisites:** [F03](F03-react-map.md), [F01](F01-build-environment.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D002](../discovery/D002-rust-host.md), [D003](../discovery/D003-android-prerequisites.md), [D014](../discovery/D014-desktop-guard-phasing.md), [D015](../discovery/D015-operation-decoding-and-admission.md). Shared guard API/integration obligations: [OPERATIONS.md](../OPERATIONS.md).

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
- [x] Local workspace startup/restart errors and remote-coordinator availability are visible; native close refuses active domain work. Real phone/transfer registration and close-warning journeys remain required before those features are enabled in F05/F08/F09.
- [x] The legacy database/reference remains recoverable; no unauthenticated LAN binding or frontend arbitrary shell access is introduced.

### Verification

Native launch/restart and lifecycle smoke journey plus targeted capability/error checks; no full installer cycle yet.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T008, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **passed for T008** in [Windows acceptance](../evidence/F04-desktop-acceptance.md); remote work/device proof remains explicitly staged. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record platform/lifecycle blockers; do not paper over failures with the Python app while claiming Rust completion.

## T009 — Implement native Android client adapters

<!-- atlas-ticket: {"id":"T009","feature":"F04","status":"ready","depends_on":["T002","T003","T007"],"evidence":null} -->

### Outcome and included work

Run the compact shared React interface in a native Android Tauri app with correct platform adapters.

### Exclusions

Direct phone SSH key storage, Android shell execution, Play Store/iOS work or final pairing claims.

### Implementation and contracts

Implement the approved separate compact phone composition and touch navigation/pan/zoom without overwriting desktop coordinates (moved intact from T006/T007 by the owner's desktop-first priority on 2026-10-25). Reuse desktop components and contracts. Implement coordinator transport interface, protected credential-store adapter and Android scoped picker/URI streaming. Do not assume filesystem paths from phone URIs. Guard mobile lifecycle and network errors; use controlled transport fixtures until real pairing passes.

**Ticket dependencies:** [T002](F01-build-environment.md), [T003](F02-domain-persistence.md), [T007](F03-react-map.md).

### Acceptance criteria

- [ ] Native Android shell renders approved UI and uses the shared operation contracts.
- [ ] Separate compact phone composition at 320/390px is readable, accessible and overflow-free with large saved inventories; search/focus and touch pan/zoom work without changing desktop positions or nested workload identity.
- [ ] File-picker/URI streams and protected credential storage are exercised through native adapters without broad all-files access or plaintext credentials.
- [ ] Disconnect/background/resume states are explicit; fixture transport cannot be mistaken for successful production authentication.

### Verification

One compact-phone journey at 320/390px, including touch navigation/pan/zoom and independent arrangement; one Android native adapter smoke test/build, including picked-file URI and lifecycle error path. Real pairing is F05 and physical acceptance T026.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T009, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record missing native/device facilities instead of substituting browser-only proof.

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
