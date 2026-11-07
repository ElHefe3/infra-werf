# Live infrastructure awareness — proposed extension

Date: 2026-10-27<br>
State: dynamic server lifecycle requested; runtime/deployment choice pending

## Owner request

The main diagram must reveal unreachable servers and current system changes, rather than presenting saved inventory as a live operational picture. Extend useful discovery and monitoring while retaining the existing access tools. Mobile remains future work.

Owner clarification: servers are dynamic resources that come and go. Useful history and deliberate ways to remove resources are part of the product's normal lifecycle.

## Dynamic resource lifecycle

| Situation | Expected behavior |
| --- | --- |
| A new server is discovered | Surface a candidate with its discovery source and observation time. Establish trusted identity before enrolment or protected collection. Discovery uses configured sources/approved network boundaries. |
| A monitored server disappears | Mark it unreachable after the configured failure threshold; show last contact and retain its historical observations. Disappearance alone never deletes it. |
| The same server returns | Reconcile its trusted identity with the existing resource, restore current status and record recovery without creating a duplicate. |
| An address or hostname changes | Update connection details only when identity is verified. IP addresses and display names are not sufficient identity. A changed SSH host key still requires review. |
| A server is retired | Archive it out of the active diagram while retaining its timeline. Allow deliberate restoration. A later sighting of an archived resource must be surfaced, not silently duplicated. |
| A server should no longer be tracked | Ignore it and stop checks/collection; retain only the information needed for the chosen history/rediscovery policy. Make the ignored state visible and reversible. |
| The owner wants records erased | Offer permanent local deletion with a clear preview of server history and diagram relationships affected. Explain whether future discovery may offer it again. This does not uninstall or alter the remote machine. |

Keep resource lifecycle (candidate, active, archived, ignored) separate from observed health (checking, reachable, unreachable, unknown, stale). The active map emphasizes current health; history explains when the server appeared, what changed, when contact was lost and when it recovered.

History retention must be bounded and configurable. Show timestamps, discovery/collection source and incomplete observations. Record meaningful changes rather than appending identical polling results indefinitely. Removing a server or its history is an explicit owner action.

## Verified current gap

Server cards use inventory freshness for their main status dot and show saved workload states. Network checks require per-host configuration, and saved configurations are restored when the Health panel mounts rather than centrally at application startup. The main diagram therefore does not provide automatic fleet monitoring. Server 1 remains owner-confirmed powered off/corrupt and excluded from contact until that instruction changes.

## First functional batch

- Make host status prominent on each map card: reachable, unreachable, checking, unknown, stale, or owner-reported offline. Show the last check and last successful contact.
- Keep host contact, service health and inventory freshness separate. An unreachable SSH port does not by itself establish that the machine is powered off.
- Start configured checks centrally, independent of which workspace tab is open. Persist configuration and bounded observations; retain explicit monitoring enable/disable controls.
- Discover approved baseline services, containers and resource observations periodically through the chosen authenticated collection method.
- Record useful transitions: contact lost/restored, workload added/removed/state changed, and configured resource thresholds. Retain bounded history and show when a result is incomplete.
- Include candidate review, archive/restore, ignore/unignore and local deletion in the server lifecycle; preserve identity across ordinary disappear/reappear and address changes.
- Keep old inventory visible as historical data, with its collection time and status clearly attached. Validate healthy-to-unreachable-to-recovered behavior using controlled fixtures.

## Server collector option

A small restricted collector can gather approved hostname/resource/service/container/port fields without granting owner, SSH or sudo authority. It needs explicit host installation/removal policy, authenticated enrolment/revocation, bounded buffering and a selected receiver. Protected files, logs, configuration and control operations remain separate authenticated owner actions. No automatic service restarts or infrastructure changes are included.

## Runtime decision

Does monitoring need to keep running and recording changes when the Windows PC is off? If yes, select an always-on receiver/retention arrangement and collectors on approved running hosts. If no, central PC-driven checks and discovery can operate while InfraWerf is open. This decision controls deployment; neither server installation nor a new always-on host is presumed here.

S07's existing collector tickets provide reusable boundaries. Its old mobile/coordinator dependencies must be reviewed against the chosen desktop monitoring design before activation; this proposal does not mark deferred work implemented.
