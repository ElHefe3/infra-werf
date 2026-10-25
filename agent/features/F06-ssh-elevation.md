# F06 — Host-verified, scoped SSH access and deliberate sudo without credential leakage.

<!-- atlas-feature: {"id":"F06","scope":"S03","depends_on":["F05","F02"],"deferred":false} -->

## Work packet

**Outcome:** Host-verified, scoped SSH access and deliberate sudo without credential leakage.

**User journey:** Approve permitted operation → verify host key → authenticate existing identity → inspect → elevate one action → expire/lock and deny reuse.

**Scope:** [S03](../scopes/S03-secure-connections.md). **Feature prerequisites:** [F05](F05-device-pairing.md), [F02](F02-domain-persistence.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

No seed discovery applies beyond the shared scope. Link new findings when material.

## T012 — Resolve SSH identities and verify host trust

<!-- atlas-ticket: {"id":"T012","feature":"F06","status":"todo","depends_on":["T008","T014"],"evidence":null} -->

### Outcome and included work

Use system OpenSSH configuration and the local SSH agent with explicit host-key verification.

### Exclusions

Password/key onboarding, broad known-host reset or unauthorised changes to real host trust.

### Implementation and contracts

Resolve known concrete host aliases/user/port/identity references using OpenSSH. Preserve existing configuration; do not read private key contents or disable strict host trust. Present unknown/changed fingerprints and require deliberate trust handling before authentication.

**Ticket dependencies:** [T008](F04-native-shells.md), [T014](F06-ssh-elevation.md).

### Acceptance criteria

- [ ] Existing configured identities/agent are usable without moving keys into UI/database.
- [ ] Unknown and changed host identities prevent protected operations until explicitly resolved; errors distinguish trust, authentication and reachability.
- [ ] User/port/host arguments are validated, secrets are redacted and existing known-host/config files are not silently overwritten.

### Verification

Controlled SSH fixture tests for trusted/unknown/changed key, unavailable agent and rejected authentication; one native inspection entry journey.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T012, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop on unexpected trust changes or unavailable identity; never auto-accept host keys.

## T013 — Implement protected SSH sessions and per-action sudo

<!-- atlas-ticket: {"id":"T013","feature":"F06","status":"todo","depends_on":["T012","T011"],"evidence":null} -->

### Outcome and included work

Provide short-lived protected inspection and fresh explicit elevation for each privileged operation.

### Exclusions

Permanent root shortcuts, modifying sudoers, silently reusing remote cached elevation or live privileged changes without authorisation.

### Implementation and contracts

Default SSH session idle timeout is five minutes and absolute duration 30 minutes; lock/revoke closes it sooner. Sudo prompts name server/action/path, passwords stay only in the active flow and are not reused/saved/logged. Respect remote timestamp behavior without changing global policy or unrelated sessions.

**Ticket dependencies:** [T012](F06-ssh-elevation.md), [T011](F05-device-pairing.md).

### Acceptance criteria

- [ ] Protected results are fetched only after SSH authentication and cleared at session expiry/lock/revoke.
- [ ] Every privileged action requires its own visible approval/password flow; wrong-password/cancel/failure leaves no reusable secret or privileged handle.
- [ ] Expired handles cannot read/mutate; persistent public inventory and logs contain no protected results or passwords.

### Verification

Focused expiry/cancel/denial and secret-retention tests with a controlled sudo fixture; inspect persistent/log outputs using synthetic secrets. Obtain bounded review when available.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T013, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Block elevation if fresh-auth behavior or result clearing is unproven; do not weaken the requirement to finish the batch.

## T014 — Enforce safe operation and output boundaries

<!-- atlas-ticket: {"id":"T014","feature":"F06","status":"todo","depends_on":["T003","T010"],"evidence":null} -->

### Outcome and included work

Implement the shared backend authorisation and argument boundary before enabling SSH-backed operations.

### Exclusions

Generic remote scripting API, unrestricted Tauri shell permissions or bypasses for convenience.

### Implementation and contracts

Execute typed allowlisted operation requests, validate server/session/workload/path ownership, use bounded time/output and safe process argument handling, and separate terminal input from file/service commands. Return structured redacted failures; no arbitrary frontend shell command for managed operations.

**Ticket dependencies:** [T003](F02-domain-persistence.md), [T010](F05-device-pairing.md).

### Acceptance criteria

- [ ] Unauthenticated, revoked, mismatched-host/session and unknown operation requests are denied centrally for both transports.
- [ ] Adversarial IDs/paths/arguments cannot escape the allowed operation or inject shell syntax; output/time limits fail predictably.
- [ ] Logs/audit/progress omit credentials and protected payloads; operation cancellation/retry cannot duplicate a mutation silently.

### Verification

Boundary/negative tests for auth, argument injection, output limit and cancellation using a fake executor; repeat relevant cases against the controlled SSH fixture after T012.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T014, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop a newly discovered privilege/quoting boundary issue and add focused evidence before exposing the affected operation.
