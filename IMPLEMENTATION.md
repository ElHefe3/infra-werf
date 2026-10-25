# A01 — local coordinator and shared map

Delivered 25 October 2026. Runs on Windows PC, loopback port 8770. This implements the owner-approved first local increment, without changing homelab hosts or the existing infrastructure-maintenance repository.

## Working behavior

Desktop `/` and phone `/mobile` use the same API and SQLite database. Host registration, renaming, connection-hostname changes, desktop node layout and directional relationship creation/removal persist on the coordinator. Mobile arrangement remains compact and independent of desktop coordinates.

Four sanitized existing records were imported from `../server map/dashboard/data`: Core & data, Infrastructure, Compute & apps, Communications. The imported snapshots contain 540 service/container records, including inactive/stopped workloads. That total is not a count of distinct applications or healthy services. Original collection times and per-report warnings remain visible. All four current records are stale; this does not establish whether any host is currently online. No fabricated dependency edges were imported from the prototypes.

Importing again with unchanged source reports returns zero updates. New reports preserve owner map data. Source reports are read only and are not added to this Git repository. Passwords, private keys, raw configurations and protected files are not ingested.

## Verification

- Python tests: 10 focused checks for restart persistence, map edits and relationships, duplicate hosts, invalid coordinates/hostnames/targets, atomic bounded import and ordering, shared HTTP API/assets, same-origin restrictions, private asset denial and the fixed import directory.
- All three JavaScript modules pass Node syntax checks.
- Browser: desktop at 1280×800 and mobile at 390×844; server workspace checked at 320×740, with no horizontal document overflow.
- Browser: real saved workload searches; saved state and observation date shown alongside results.
- Browser: duplicate `lab-server-1` registration reports an error and creates no extra node.
- Browser: desktop node dragged, saved and reloaded; inline world position persisted (`89.1667, 77.6190`).
- Browser: unchanged inventory import reports `0 newer saved reports imported`.
- Browser: workspace tab widths corrected after narrow-phone review; server titles keep useful space.
- Browser error log empty during the final desktop check.

Touch gesture handling supports pan and two-pointer zoom; actual hardware pinch gestures and physical-phone access are not yet verified. This increment has no network-facing owner authentication. It binds only to loopback.

## Next

This document is historical A01 evidence. The current [agent roadmap](agent/AGENT.md) and [scope](agent/SCOPE.md) govern new work: latest-stable mise foundations, Rust/React parity and native Windows/Android shells precede authenticated pairing, SSH, files and operations. Always-on deployment is deferred; the first coordinator runs on the PC. No new tracker ticket is complete because A01 existed.
