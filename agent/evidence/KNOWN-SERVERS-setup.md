# Known lab server setup

Date: 2026-10-27. Result: pass for the requested four-server configuration in the owner's active Windows InfraWerf window.

- Preserved the existing lab-server-2 record and its corrected Host alias.
- Added lab-server-3, lab-server-4 and lab-server-1 using the identically named OpenSSH aliases, rather than raw IP addresses.
- Saved lab-server-1 as owner-reported offline with automatic checks and inventory collection disabled. No connection or collection was requested for it.
- Enabled automatic reachability/SSH verification every 60 seconds and approved baseline inventory collection every 900 seconds on lab-server-2, lab-server-3 and lab-server-4.
- Observed all three online servers display Reachable. Native UI showed automatic inventory collection complete and three saved inventories containing 385 workloads in total.
- Returned to Topology and used Fit map. Confirmed the four lab cards: three Reachable, one Offline / owner reported.
- The pre-existing git-host and spare-pc entries remain inactive candidates; neither was activated or contacted during this setup. No discovery action, SSH trust change, credential change or remote mutation was performed.

Monitoring runs while InfraWerf is open. This setup record does not resolve the earlier installer checksum or workspace-location discrepancy recorded in D048, and does not close all F16 release acceptance work.
