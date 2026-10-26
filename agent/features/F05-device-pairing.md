# F05 — An explicitly paired, encrypted and revocable Android connection to the Windows coordinator.

<!-- atlas-feature: {"id":"F05","scope":"S03","depends_on":["F04"],"deferred":false} -->

## Work packet

**Outcome:** An explicitly paired, encrypted and revocable Android connection to the Windows coordinator.

**User journey:** Display pairing request → confirm PC identity/device on desktop → authenticate phone → revoke/expire → deny further access.

**Scope:** [S03](../scopes/S03-secure-connections.md). **Feature prerequisites:** [F04](F04-native-shells.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

No seed discovery applies beyond the shared scope. Link new findings when material.

## T010 — Implement encrypted pairing and device revocation

<!-- atlas-ticket: {"id":"T010","feature":"F05","status":"todo","depends_on":["T008","T009"],"evidence":null} -->

### Outcome and included work

Pair the Android app with the PC over LAN/existing VPN without exposing unauthenticated operations.

### Exclusions

Public hosting, VPN installation, account/team identity providers or publishing pairing secrets.

### Implementation and contracts

Use TLS with explicit certificate trust/pinning, a cryptographically random single-use pairing secret expiring after five minutes, visible desktop approval and revocable per-device identity. Keep secrets in protected OS storage and out of QR screenshots/evidence/logs.

Register active authenticated remote work with T008's shared lifecycle admission guard before enabling it. Verify ordinary desktop close/quit shows an explicit warning and preserves active phone work; transfer jobs in F08/F09 must retain registration for the full background job, not merely its initial request. See [D013](../discovery/D013-native-desktop-proof.md) and [Windows lifecycle evidence](../evidence/F04-desktop-acceptance.md).

**Ticket dependencies:** [T008](F04-native-shells.md), [T009](F04-native-shells.md).

### Acceptance criteria

- [ ] Only explicit unexpired pairing and desktop approval grant access; replay/expiry/wrong-PC identity fail.
- [ ] Wrong or changed server identity cannot be bypassed by a global TLS-validation disable.
- [ ] Revoking a device denies subsequent requests and closes its active channels; no unauthenticated public/LAN mutation endpoint exists.
- [ ] Paired Files uses T014/T015's shared guarded session path; compact-phone selection/breadcrumbs work, and lock/revoke/expiry clears listings and denies stale handles. Native-only proof does not satisfy this phone journey.

### Verification

Focused trust/pair/replay/revocation integration tests and one Windows/Android pairing journey on the private connection. Obtain bounded review when available.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T010, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Pause exposure if trusted transport/OS storage cannot be proved; do not use cleartext or static shared passwords as shortcuts.

## T011 — Implement session expiry and reconnect

<!-- atlas-ticket: {"id":"T011","feature":"F05","status":"todo","depends_on":["T010"],"evidence":null} -->

### Outcome and included work

Maintain explicit device sessions with safe expiry, lock and reconnect behavior.

### Exclusions

Changing SSH/sudo policy, silently extending credential lifetime or offline privileged work.

### Implementation and contracts

Default application-session idle timeout is 15 minutes and absolute duration eight hours; pairing identity does not itself confer protected SSH access. On disconnect/background/lock clear protected views and stop terminal input; reconnect reauthenticates and does not replay mutations automatically.

**Ticket dependencies:** [T010](F05-device-pairing.md).

### Acceptance criteria

- [ ] Idle/absolute expiry, device revoke and wrong session consistently reject requests/channels.
- [ ] Phone lock/background/disconnect clears protected results and reconnect never resurrects expired SSH/operation handles.
- [ ] Approved baseline offline state remains visibly timestamped; no mutation is silently replayed and duplicate operations are avoided.

### Verification

Clock-controlled session tests, interrupted-connection integration and one native background/resume journey.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T011, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record lifecycle/transport ambiguity before allowing resumable access across expired sessions.
