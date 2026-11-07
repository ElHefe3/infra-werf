# F16 — Dynamic desktop monitoring and server lifecycle

<!-- atlas-feature: {"id":"F16","scope":"S10","depends_on":["F07","F12"],"deferred":false} -->

Outcome: status on the map reflects current checks, configured servers refresh their inventory periodically, and retired servers retain useful history.

Boundary: monitoring runs while the Windows app is open. Existing system OpenSSH trust applies. No LAN sweep, agent deployment, remote modification, mobile work or contact with owner-reported offline servers. Discovery reads explicit aliases in the local SSH configuration; candidates require review before checks. No address-based automatic identity merge.

## T034 — Desktop monitoring and lifecycle

<!-- atlas-ticket: {"id":"T034","feature":"F16","status":"in_progress","depends_on":["T019","T020","T025"]} -->

### Outcome and included work

Persistent desktop monitoring, meaningful history and local server lifecycle management.

### Exclusions

No agent installation, LAN sweep, mobile implementation or always-on receiver deployment.

### Implementation and contracts

Opt-in SSH checks and approved baseline collection run while the app is open. Local SSH configuration discovery creates candidates atomically. Policy and endpoint changes drain the current bounded contact before acknowledging changes. Store schema 3 preserves a complete schema-2 recovery copy. Changed identities require explicit review and fresh trusted verification.

### Acceptance criteria

- [ ] Persistent, opt-in bounded recurring SSH checks and baseline collection, independent of the selected tab.
- [ ] Prominent reachable/unreachable/unknown/stale status separate from saved inventory; owner-reported offline excludes contact.
- [ ] Timestamped state transitions and workload changes; bounded history with retention control, no unchanged refresh noise.
- [ ] Configured-source candidates, explicit activation, archive/ignore/restore and permanent local deletion preview.
- [ ] Safe native-store upgrade with a pre-upgrade backup; restart and failure/recovery proof on controlled data.

### Verification

Proof: focused storage/scheduler tests, TypeScript/Rust checks, native UI journey and independent review. Build/install only after these pass. Record limits and actual evidence before closure.

### Evidence and closure

In progress. No acceptance claimed yet.

### Stop conditions

Stop condition: this desktop journey is implemented and tested; always-on infrastructure remains future work.
