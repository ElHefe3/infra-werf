# D067 — An owner PIN hides sensitive details and gates protected views

Date: 2026-11-18. Related: S14 / F27 T065, T064, [D066](D066-cached-app-discovery-and-icons.md).

## Owner decision

On 2026-11-18 the owner asked to "hide sensitive data and request a pin every time it gets viewed". When asked, they chose:

- **What is hidden:** application details (app names stay; ports, addresses, process names, page titles and versions sit behind an ⓘ button); protected views (Files, Terminal, live workload details and application windows); and host key fingerprints.
- **When it is asked:** once per few minutes, not on every view.
- **Which PIN:** InfraWerf's own PIN, not Windows Hello.

## What was built

- **Core gate.** `atlas-core/src/pin.rs` keeps a salted PBKDF2-HMAC-SHA256 hash (310,000 iterations, via the already compiled `ring`). A correct PIN unlocks for 5 minutes; after 5 wrong PINs each further try waits 30 s longer. The count is kept in `pin-attempts.json`, so restarting InfraWerf does not give back free tries (added 2026-11-19 after review). Files, Terminal, workload inspection, tunnel and plugin-app commands refuse with `pin_required` while a PIN is set and locked. This follows the rule that protected information is gated where it is fetched, not only hidden.
- **Storage.** The hash is in `pin.json` beside the workspace database; the database is unchanged. A damaged file counts as a set PIN that nothing matches, so it never silently removes the lock. Deleting the file (with InfraWerf closed) removes the PIN.
- **Interface.** A refused command opens the PIN prompt and retries once. Fingerprints and app details are masked on screen until unlocked and hide again when the unlock ends. Settings → PIN sets, changes or removes it, and a "Lock now" button appears while unlocked.

## Limits

- It is a privacy lock against someone else at this PC, not encryption. A 4–12 digit PIN cannot resist an attacker who has the file and time.
- Fingerprints and app details are already in the interface's memory from authenticated sessions or the cache; for those the PIN controls display only.
- Sessions opened while unlocked keep working when the 5-minute unlock simply runs out, so a long Terminal session is not cut off. "Lock now" closes every open Files, Terminal and workload-details session and every application window (added 2026-11-19 after review).
- Someone who can delete files in the owner's Windows account can remove `pin.json` or `pin-attempts.json`; the PIN does not defend against that.
