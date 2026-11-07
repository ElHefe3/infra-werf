# F17 — Deferred remote applications through authenticated tunnels

<!-- atlas-feature: {"id":"F17","scope":"S11","depends_on":["F06","F12"],"deferred":true} -->

## Work packet

**Outcome:** Open a server's private web interfaces (for example a dashboard bound to `127.0.0.1:8080` on the server) in an InfraWerf window, and optionally run reviewed graphical remote applications, without exposing them on the network.

**User journey:** Select a server → choose or enter a remote `localhost` port → InfraWerf verifies SSH access and opens a local-only tunnel → an isolated in-app browser window shows the application → closing the window, Lock or exit removes the tunnel.

**Scope:** [S11](../scopes/S11-remote-applications.md). **Feature prerequisites:** [F06](F06-ssh-elevation.md), [F12](F12-release-acceptance.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation.

Context: the owner requested this on 2026-11-07 as a potential feature ("launching applications as if native", such as a browser reaching the server's localhost). It is captured here and is not authorised for implementation.

## T035 — Forwarded web view for remote localhost services

<!-- atlas-ticket: {"id":"T035","feature":"F17","status":"deferred","depends_on":["T033","T021","T026"],"evidence":null} -->

### Outcome and included work

An owner-started SSH local forward (`-L 127.0.0.1:<ephemeral>:127.0.0.1:<remote-port>`) bound to the selected server profile, displayed in a separate isolated Tauri webview window with address bar, reload, open-in-system-browser and a visible tunnel indicator.

### Exclusions

LAN/0.0.0.0 binding, remote or dynamic forwarding, arbitrary destination hosts beyond the server's own loopback (initially), persistent background tunnels, credential autofill or saving remote site passwords.

### Implementation and contracts

Reuse the system OpenSSH child with strict host checking and the existing profile/identity; allow only the explicit forward (`ExitOnForwardFailure=yes`, no remote command, no agent/X11 forwarding). Choose an ephemeral local port and pass it only to the owned webview. The remote-content webview must have no Tauri IPC/capabilities, a separate data directory, and navigation limited to the tunnel origin unless the owner opens a link externally. Track tunnels under the existing session lifecycle: cancel, Lock, idle/absolute expiry and app exit reap the child. Optionally remember per-server named shortcuts (name + remote port only, no secrets). Record the forwarding boundary change as an owner decision before starting.

**Ticket dependencies:** [T033](F04-native-shells.md), [T021](F10-terminal.md), [T026](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Forward listens only on Windows loopback and only for the selected server/profile; a port already in use or a failed remote bind reports a clear error without fallback.
- [ ] HTTP and WebSocket applications work in the isolated window; that window cannot invoke InfraWerf commands.
- [ ] Close, Lock, expiry, server lifecycle change and app exit terminate the tunnel; no listener survives.
- [ ] Unknown/changed host keys and authentication failures use the existing trust/review outcomes; nothing alters OpenSSH trust.

### Verification

Loopback asyncssh/OpenSSH fixture with a private HTTP+WebSocket app on the fixture's loopback; native Windows journey plus a listener scan after each teardown path. Independent review of the webview isolation and forwarding options.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md) mapped to T035. Current proof: **not run**.

### Stop conditions

Remain deferred until S11 activation and the recorded forwarding decision. Stop if webview isolation from IPC cannot be proven on Windows.

## T036 — Decide and prototype remote graphical applications

<!-- atlas-ticket: {"id":"T036","feature":"F17","status":"deferred","depends_on":["T035"],"evidence":null} -->

### Outcome and included work

A recorded decision on how InfraWerf could run graphical Linux applications "as if native" (candidate approaches: browser-based app servers reached through T035, xpra/Waypipe over SSH, RDP/VNC through a forwarded port), followed by one prototype if approved.

### Exclusions

Installing anything on a server or the Windows PC without per-host owner approval; always-on remote desktop services; bundling an X server implicitly.

### Implementation and contracts

Compare each approach for Windows client runtime, server-side install footprint, licensing, latency, clipboard/file boundaries and how it fits the existing SSH authority and Lock model. Prefer approaches that need no server install or reuse T035's tunnel. Record the choice in a discovery note and pin any runtime through mise.

**Ticket dependencies:** [T035](F17-remote-applications.md).

### Acceptance criteria

- [ ] Decision record lists approaches, install implications and the owner's choice.
- [ ] If a prototype is approved: one reviewed application launches in its own window over the authenticated tunnel and is torn down by close/Lock/exit.
- [ ] Clipboard and file exchange are off by default and explicitly enabled per session.

### Verification

Narrow controlled-fixture prototype only after the decision; no broad tool comparison loop.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md) mapped to T036. Current proof: **not run**.

### Stop conditions

Stop if the chosen approach requires server installation without approval or weakens the existing SSH/Lock boundaries.
