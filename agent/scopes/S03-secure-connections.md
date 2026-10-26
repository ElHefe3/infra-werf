# S03 — Secure connections

<!-- atlas-scope: {"id":"S03","features":["F05","F06"],"depends_on":["S02"],"window":1,"deferred":false} -->

## Outcome

A trusted, paired and revocable Android-to-PC connection with host-verified SSH and deliberately scoped elevation.

## Entry and sequence

Full-scope entry requires S02 complete and controlled trust/SSH/sudo fixtures available. T014 explicitly permits desktop-first entry over T008/T032/T033; this does not pass the paired/phone/sudo scope gates. Actual installation/trust/credential actions require their applicable authorisations.

Complete F05 pair/session behavior. Within F06, T014 operation boundaries precede T012 SSH resolution, then T013 protected sessions/sudo. Follow dependencies, not ticket numbering.

**Scope dependencies:** [S02](S02-applications-map.md). **Budget:** window 1.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F05](../features/F05-device-pairing.md) | T010, T011 | Display pairing request → confirm PC identity/device on desktop → authenticate phone → revoke/expire → deny further access. |
| [F06](../features/F06-ssh-elevation.md) | T012, T013, T014 | Approve permitted operation → verify host key → authenticate existing identity → inspect → elevate one action → expire/lock and deny reuse. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] TLS identity, explicit pairing, single-use expiry, revocation and protected credential storage pass positive/negative tests.
- [ ] App-session expiry/reconnect/background handling does not replay mutations or resurrect protected sessions.
- [ ] Existing agent identities authenticate without key copying; unknown/changed SSH host identity stops access.
- [ ] Per-action fresh sudo, secret redaction, protected-result clearing and safe argument/output boundaries are proven; bounded auth review is recorded when available.

## Verification and refinement

Clock-controlled auth and adversarial input fixtures plus one native private-network pairing/SSH journey. Run relevant shared auth/contract regression here; do not repeat unaffected visual suites.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md) and do not record native/owner tests as passed if unavailable.

## Exclusions and stop boundary

Public hosting, new VPN setup, persistent privileged exceptions, automatic host-key acceptance or changing sudoers/global sudo policies.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work. Do not silently reduce acceptance criteria or bypass prerequisites to meet the usage target.

## Exit

S04 by default; S05 is independently eligible after S03 if file work is blocked. Save a window-1 restart checkpoint and reassess the budget honestly.
