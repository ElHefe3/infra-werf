# D047 — Desktop monitoring policy and observation lifecycle

Date: 2026-10-27. Related: F16 / T034.

Saved workload states cannot establish current reachability. InfraWerf now stores monitoring policy and timestamped observations separately, and ages status during rendering even when IPC polling fails. Owner-reported offline is an explicit policy that excludes contact; it is not fabricated network evidence.

Policy saves, endpoint edits and deletion drain the current bounded background contact before acknowledging the new generation. A compare-and-set on host/revision also rejects late persistence. Legacy service probes recheck policy at admission. No network operation holds the SQLite store mutex.

Store schema 2 upgrades to 3 only after a complete, atomically published `pre-monitoring-*.sqlite3` recovery copy. The exclusive SQLite ownership lock remains held throughout backup and transactional upgrade. Candidate creation includes its inactive policy in the same transaction; partial writes must never expose an active candidate.

The initial native journey found a recursive store lock in startup retry when applying persisted status. Releasing the startup guard before the status overlay fixes it; successful retry is now covered by the native workspace regression. Native command tests have explicit time limits so such regressions fail visibly.

Discovery reads literal aliases from the primary OpenSSH configuration only. Includes, wildcard entries and network scanning are excluded. A deleted alias can reappear as a fresh inactive candidate; ignore retains its existing record and suppresses checks. Address/name changes never merge IDs. A replacement key requires explicit same-server review plus fresh verification under existing OpenSSH trust before inventory may replace saved observations.

Monitoring runs while the desktop is open. Always-on hosting and remote collector deployment remain separate future decisions.
