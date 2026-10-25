# D009 — Tracker self-tests must control their ticket preconditions

Date: 2026-10-25. State: resolved. Affected: T001 verification and subsequent ticket closure.

## Finding

The actual tracker validated after T001 was completed, but its in-memory negative self-test failed. The incomplete-dependency fixture only changed T002 to ready; after T001 became done, that was a valid state. The unchecked-acceptance fixture also depended on the live T001 checklist remaining unchecked.

## Evidence

`mise run tracker:check` first reported a valid 31-ticket tracker, then raised `Rejection fixture failed: incomplete dependency: []`. The fixture's intended invalid precondition had disappeared as legitimate project progress occurred.

## Consequence

Self-tests now normalize ticket state/evidence and checklists in memory before applying each controlled mutation. The real tracker is still validated as-is first. No project files are changed by the checker, and no evidence requirement is weakened.

## Follow-up

All ten rejection fixtures and the valid-completion fixture passed against the progressed tracker with T001 done. Keep future test preconditions independent of live completion state.
