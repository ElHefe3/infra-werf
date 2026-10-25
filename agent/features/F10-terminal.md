# F10 — An explicitly authenticated interactive terminal that behaves correctly in desktop and Android lifecycles.

<!-- atlas-feature: {"id":"F10","scope":"S05","depends_on":["F06","F04"],"deferred":false} -->

## Work packet

**Outcome:** An explicitly authenticated interactive terminal that behaves correctly in desktop and Android lifecycles.

**User journey:** Authenticate → open terminal → type/resize/interrupt → disconnect/lock → reject stale input and reauthenticate.

**Scope:** [S05](../scopes/S05-observe-operate.md). **Feature prerequisites:** [F06](F06-ssh-elevation.md), [F04](F04-native-shells.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

No seed discovery applies beyond the shared scope. Link new findings when material.

## T021 — Implement authenticated interactive terminal

<!-- atlas-ticket: {"id":"T021","feature":"F10","status":"todo","depends_on":["T013","T014","T009","T007"],"evidence":null} -->

### Outcome and included work

Provide interactive SSH terminal input/output and resize on both clients.

### Exclusions

Remote desktop streaming, persistent shell recording or bypassing managed-operation checks through a hidden terminal.

### Implementation and contracts

Use a bounded PTY/channel with a terminal UI suited to desktop and Android keyboards. Tie the terminal to server/device/protected-session identity; enforce output backpressure, close on expiry/revoke/lock and never replay buffered input after reconnect.

**Ticket dependencies:** [T013](F06-ssh-elevation.md), [T014](F06-ssh-elevation.md), [T009](F04-native-shells.md), [T007](F03-react-map.md).

### Acceptance criteria

- [ ] Interactive input, Unicode, control/interrupt keys, resize and Android keyboard behavior pass a controlled session journey.
- [ ] Slow/large output remains bounded and disconnect/lock/expiry reliably stops input/output and clears protected content.
- [ ] A new connection requires fresh valid handles; stale input cannot reach another session/host and terminal contents are not persisted or logged.

### Verification

Focused PTY/transport lifecycle checks plus one Windows and actual Android terminal journey; synthetic output for buffer/secret tests.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T021, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Block unsupported PTY/keyboard/lifecycle behavior rather than shipping a command box labelled as a working terminal.
