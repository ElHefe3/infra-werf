# F04 — Real native Windows and Android clients with a shared domain adapter and platform-specific lifecycle/I/O.

<!-- atlas-feature: {"id":"F04","scope":"S02","depends_on":["F03","F01"],"deferred":false} -->

## Work packet

**Outcome:** Real native Windows and Android clients with a shared domain adapter and platform-specific lifecycle/I/O.

**User journey:** Launch Windows coordinator → open both native clients → navigate the shared map/workspace → handle lifecycle without losing work.

**Scope:** [S02](../scopes/S02-applications-map.md). **Feature prerequisites:** [F03](F03-react-map.md), [F01](F01-build-environment.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D002](../discovery/D002-rust-host.md), [D003](../discovery/D003-android-prerequisites.md).

## T008 — Integrate Windows Tauri and coordinator lifecycle

<!-- atlas-ticket: {"id":"T008","feature":"F04","status":"todo","depends_on":["T002","T003","T004","T007"],"evidence":null} -->

### Outcome and included work

Run the Rust application/domain in the Windows Tauri shell with explicit local coordinator lifecycle.

### Exclusions

Always-on server deployment, automatic startup service installation or public network exposure.

### Implementation and contracts

Replace dependency on the Python foreground/background launcher in the new build. Restrict native capabilities; route commands through the same domain operations. Show coordinator availability, handle single-instance/close/quit behavior and active remote-work warnings without silently killing transfers.

**Ticket dependencies:** [T002](F01-build-environment.md), [T003](F02-domain-persistence.md), [T004](F02-domain-persistence.md), [T007](F03-react-map.md).

### Acceptance criteria

- [ ] Native Windows app loads real persisted data and invokes restricted Rust domain commands.
- [ ] Coordinator startup/shutdown/restart errors are visible; ordinary window close does not silently terminate active phone/transfer work.
- [ ] The legacy database/reference remains recoverable; no unauthenticated LAN binding or frontend arbitrary shell access is introduced.

### Verification

Native launch/restart and lifecycle smoke journey plus targeted capability/error checks; no full installer cycle yet.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T008, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record platform/lifecycle blockers; do not paper over failures with the Python app while claiming Rust completion.

## T009 — Implement native Android client adapters

<!-- atlas-ticket: {"id":"T009","feature":"F04","status":"todo","depends_on":["T002","T003","T007"],"evidence":null} -->

### Outcome and included work

Run the compact shared React interface in a native Android Tauri app with correct platform adapters.

### Exclusions

Direct phone SSH key storage, Android shell execution, Play Store/iOS work or final pairing claims.

### Implementation and contracts

Implement coordinator transport interface, protected credential-store adapter and Android scoped picker/URI streaming. Do not assume filesystem paths from phone URIs. Guard mobile lifecycle and network errors; use controlled transport fixtures until real pairing passes.

**Ticket dependencies:** [T002](F01-build-environment.md), [T003](F02-domain-persistence.md), [T007](F03-react-map.md).

### Acceptance criteria

- [ ] Native Android shell renders approved UI and uses the shared operation contracts.
- [ ] File-picker/URI streams and protected credential storage are exercised through native adapters without broad all-files access or plaintext credentials.
- [ ] Disconnect/background/resume states are explicit; fixture transport cannot be mistaken for successful production authentication.

### Verification

One Android native adapter smoke test/build, including picked-file URI and lifecycle error path; real pairing is F05 and physical acceptance T026.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T009, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record missing native/device facilities instead of substituting browser-only proof.
