# F06 — Host-verified, scoped SSH access and deliberate sudo without credential leakage.

<!-- atlas-feature: {"id":"F06","scope":"S03","depends_on":["F04","F02"],"deferred":false} -->

Current owner decision (2026-10-26): desktop is the complete release target. Phone/coordinator/APK requirements are preserved in [S09](../scopes/S09-future-mobile.md), separately deferred. All desktop batches are authorised sequentially.

## Work packet

**Outcome:** Host-verified, scoped SSH access and deliberate sudo without credential leakage.

**User journey:** Approve permitted operation → verify host key → authenticate existing identity → inspect → elevate one action → expire/lock and deny reuse.

**Scope:** [S03](../scopes/S03-secure-connections.md). **Feature prerequisites:** [F04](F04-native-shells.md), [F02](F02-domain-persistence.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

Relevant discoveries: [D014 desktop phasing](../discovery/D014-desktop-guard-phasing.md), [D015 decoding/admission](../discovery/D015-operation-decoding-and-admission.md), [D017 feature entry](../discovery/D017-desktop-feature-entry.md), [D018 SFTP lifecycle](../discovery/D018-sftp-process-and-page-lifecycle.md). Reuse [OPERATIONS.md](../OPERATIONS.md) and the proven desktop session core; paired transport and fresh sudo remain their tickets.

## T012 — Resolve SSH identities and verify host trust

<!-- atlas-ticket: {"id":"T012","feature":"F06","status":"ready","depends_on":["T008","T014"],"evidence":null} -->

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

<!-- atlas-ticket: {"id":"T013","feature":"F06","status":"todo","depends_on":["T012","T014"],"evidence":null} -->

### Outcome and included work

Provide short-lived protected inspection and fresh explicit elevation for each privileged operation.

### Exclusions

Permanent root shortcuts, modifying sudoers, silently reusing remote cached elevation or live privileged changes without authorisation.

### Implementation and contracts

Default SSH session idle timeout is five minutes and absolute duration 30 minutes; lock/revoke closes it sooner. Sudo prompts name server/action/path, passwords stay only in the active flow and are not reused/saved/logged. Respect remote timestamp behavior without changing global policy or unrelated sessions.

Reuse T014's proven desktop protected-session/clearing core. This ticket needs fresh per-action sudo; pairing is future S09. Do not duplicate session foundations or infer elevation from read-only proof.

**Ticket dependencies:** [T012](F06-ssh-elevation.md), [T014](F06-ssh-elevation.md).

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

<!-- atlas-ticket: {"id":"T014","feature":"F06","status":"done","depends_on":["T003","T032","T033"],"scope_entry":{"S02":["T008","T032","T033"]},"evidence":"../evidence/F06-F08-desktop-acceptance.md"} -->

### Outcome and included work

Integrate T032/T033 with native protected session lifecycle and real read-only execution. Desktop-first entry uses the completed native prerequisites explicitly; paired coordinator integration and real revocation journeys remain required in T010/T011. Completion of this desktop/core slice does not complete S03's sudo gate; pairing is future S09.

### Exclusions

Generic remote scripting API, unrestricted Tauri shell permissions or bypasses for convenience.

### Implementation and contracts

Execute typed allowlisted operation requests, validate server/session/workload/path ownership, use bounded time/output and safe process argument handling, and separate terminal input from file/service commands. Return structured redacted failures; no arbitrary frontend shell command for managed operations.

**Ticket dependencies:** [T003](F02-domain-persistence.md), [T032](F04-native-shells.md), [T033](F04-native-shells.md). T010/T011 retain paired transport proof. Native/core sessions use five-minute idle and 30-minute absolute limits, lock/revoke invalidates handles and clears protected results; sudo/mutations remain unavailable until their tickets pass.

### Acceptance criteria

- [x] Unauthenticated, revoked, mismatched-host/session and unknown requests are denied by the shared guard and real native adapter; paired-adapter proof remains T010/T011.
- [x] Actual authenticated SSH/SFTP sessions enforce five-minute idle/30-minute absolute expiry and locking, with handle invalidation, child cleanup and protected-result clearing.
- [x] Adversarial IDs/paths/arguments cannot escape the allowed operation or inject shell syntax; output/time limits fail predictably.
- [x] Logs/audit/progress omit credentials and protected payloads; operation cancellation/retry cannot duplicate a mutation silently.

### Verification

Broker negative fixtures plus actual system OpenSSH/SFTP and native Files lifecycle proof. Paired-adapter/revocation proof remains T010/T011; fresh sudo remains T013.

### Evidence and closure

Passed the desktop/core slice in [T014/T015 acceptance](../evidence/F06-F08-desktop-acceptance.md), including resolved independent review and the final actual-native rerun. S03's paired/sudo acceptance remains open. Resume from [the checkpoint](../evidence/CHECKPOINT.md).

### Stop conditions

Stop a newly discovered privilege/quoting boundary issue and add focused evidence before exposing the affected operation.
