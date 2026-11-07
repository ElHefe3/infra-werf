# S11 — Deferred remote applications

<!-- atlas-scope: {"id":"S11","features":["F17"],"depends_on":["S06"],"window":null,"deferred":true} -->

## Outcome

Use a server's private applications from inside InfraWerf as if they were local: first web interfaces bound to the server's `localhost`, then, if chosen, graphical remote applications.

## Entry and sequence

The owner explicitly activates S11 after the core release. Activation deliberately widens the current SSH boundary, which excludes forwarding (see [F04](../features/F04-native-shells.md) T033). That change must be recorded as an owner decision before implementation.

Deliver the forwarded web view first (T035). Remote graphical applications (T036) start with a decision record and proceed only with separate approval of any server-side software.

**Scope dependencies:** [S06](S06-core-release.md). **Budget:** deferred; outside the core target.

## Feature batches

| Feature | Tickets | Journey |
| --- | --- | --- |
| [F17](../features/F17-remote-applications.md) | T035, T036 | Choose a server → open a remote `localhost` port in an isolated InfraWerf browser window → close/Lock tears the tunnel down; later, optionally launch a reviewed graphical application. |

Ticket status is canonical in the feature packets; scope progress is derived. Do not mark this gate complete merely because its files or implementation exist.

## Acceptance gate

- [ ] Forwarded web access binds only Windows loopback, uses existing OpenSSH trust/identity and is torn down on close, Lock, expiry and app exit.
- [ ] The embedded browser is isolated from the InfraWerf UI and IPC, and stores no remote cookies or credentials beyond the session unless the owner opts in.
- [ ] Any remote graphical application method is a recorded owner decision with its installation implications; none is installed implicitly.

## Verification and refinement

Controlled loopback SSH fixture serving a private HTTP/WebSocket application; native Windows journey covering open, navigate, reconnect, Lock and exit cleanup. No real-host testing without owner direction.

Link actual feature/gate proof and revision in [the checkpoint](../evidence/CHECKPOINT.md). Use the proportional evidence rules in [AGENT.md](../AGENT.md).

## Exclusions and stop boundary

No public or LAN-exposed listeners, remote port forwarding (`-R`), dynamic SOCKS proxying of arbitrary hosts, VPN changes, server-side installs or automatic browser credential capture.

New policy, data-preservation or permission changes stop only the affected work. Capture a discovery/blocker and continue only independently authorised ready work.

## Exit

Optional feature acceptance; no expansion into a general remote-desktop or reverse-proxy product.
