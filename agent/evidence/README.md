# Acceptance evidence and checkpoints

Create one compact `Fxx-acceptance.md` per feature when verification actually occurs. Do not pre-populate pass records. Tickets link that evidence in metadata; one feature record can cover several tickets with explicit mappings.

Required evidence sections:

- Date, revision/working-tree state, feature and included tickets.
- Effective pinned versions, platform, role, controlled fixture/target and transport.
- Acceptance mapping: each criterion, command/journey, actual result and small output/artifact reference.
- Material failures/repairs, checks not run, limitations and blockers.
- Independent review where applicable, required owner/device acceptance and rollback/cleanup results.

Mark `done` only when all ticket acceptance checkboxes are checked and an actual evidence file is linked. Native/physical-phone gates require native/device proof; source review, mocks or a browser cannot substitute. Missing environment/approval/owner decisions leave the affected ticket blocked.

The checker requires the evidence file to name each completed ticket ID, include a nonempty `Revision:` line identifying the tested revision, and include `Result: pass` only when the mapped acceptance actually passed. Set the ticket JSON comment's `evidence` value to the relative Markdown path, for example `../evidence/F01-acceptance.md`. One feature record can prove several tickets with explicit mappings; it must not claim untested criteria.

When all tickets in a scope pass, check its proven gate criteria and add an `evidence` field to that scope's JSON comment, relative to the scope file. The linked acceptance record must also name the scope ID, tested revision and result. This prevents a later scope from treating unchecked gates as completion. Run `python agent/check_tracker.py --self-test` before committing tracker changes; after T001 the same check is exposed through mise.

Update [CHECKPOINT.md](CHECKPOINT.md) at feature completion and context/usage boundaries. Keep secrets, private inventory, full logs and binaries out of tracked evidence.
