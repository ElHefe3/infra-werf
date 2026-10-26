# F10 — An explicitly authenticated interactive terminal that behaves correctly in the desktop lifecycle.

<!-- atlas-feature: {"id":"F10","scope":"S05","depends_on":["F06","F04"],"deferred":false} -->

Current owner decision (2026-10-26): desktop is the complete release target. Phone/coordinator/APK requirements are preserved in [S09](../scopes/S09-future-mobile.md), separately deferred. All desktop batches are authorised sequentially.

## Work packet

**Outcome:** An explicitly authenticated interactive terminal that behaves correctly in the desktop lifecycle.

**User journey:** Authenticate → open terminal → type/resize/interrupt → disconnect/lock → reject stale input and reauthenticate.

**Scope:** [S05](../scopes/S05-observe-operate.md). **Feature prerequisites:** [F06](F06-ssh-elevation.md), [F04](F04-native-shells.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

See [D030](../discovery/D030-windows-conpty-terminal-startup.md) for Windows ConPTY handle setup and pinned authentication evidence.

## T021 — Implement authenticated interactive terminal

<!-- atlas-ticket: {"id":"T021","feature":"F10","status":"done","depends_on":["T013","T014","T007","T008"],"evidence":"../evidence/T021-acceptance.md"} -->

### Outcome and included work

Provide interactive SSH terminal input/output and resize on Windows.

### Exclusions

Remote desktop streaming, persistent shell recording or bypassing managed-operation checks through a hidden terminal.

### Implementation and contracts

Use a bounded PTY/channel with a terminal UI suited to the desktop; Android keyboard/lifecycle remains S09. Tie the terminal to server/device/protected-session identity; enforce output backpressure, close on expiry/revoke/lock and never replay buffered input after reconnect.

**Ticket dependencies:** [T013](F06-ssh-elevation.md), [T014](F06-ssh-elevation.md), [T008](F04-native-shells.md), [T007](F03-react-map.md).

### Acceptance criteria

- [x] Interactive input, Unicode, control/interrupt keys, resize and desktop keyboard behavior pass a controlled session journey.
- [x] Slow/large output remains bounded and disconnect/lock/expiry reliably stops input/output and clears protected content.
- [x] A new connection requires fresh valid handles; stale input cannot reach another session/host and terminal contents are not persisted or logged.

### Verification

Focused PTY/transport lifecycle checks plus one actual Windows terminal journey; synthetic output for buffer/secret tests.

### Evidence and closure

See [T021 acceptance](../evidence/T021-acceptance.md) for the mapped results, tested revision/platform/fixture, repairs, limits and cleanup.

### Stop conditions

Block unsupported PTY/keyboard/lifecycle behavior rather than shipping a command box labelled as a working terminal.
