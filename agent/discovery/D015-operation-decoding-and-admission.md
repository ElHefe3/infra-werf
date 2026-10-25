# D015 — Strict tagged decoding and cancellation admission

Date: 2026-10-25. State: **resolved**. Tickets: T032, T014, T012.

## Finding

The first negative guard test showed Serde's internally tagged unit variant accepted extra fields despite the enum's deny-unknown-fields annotation. An empty struct variant enforces the intended shape. Generated TS types and ts-rs warnings do not prove Rust input validation.

Independent review also found a start race: a previously validated prepared job could be cancelled/revoked before the worker claimed it, then still invoke its callback. Checking only result delivery prevented payload release but did not prevent new work after cancellation.

## Evidence

`crates/atlas-core/src/operations.rs` uses a bounded redacted decoder and an empty SystemSummary struct variant. Admission revalidates under the same registry→phase lock order as cancellation/delivery. Execution context checkpoints again before callback entry. `mise run test:feature -- T032` passed 2 deterministic admission/entry regressions and 9 guard integration tests. The reviewer confirmed both repairs resolved. A worker recovery assertion now waits for actual worker cleanup rather than assuming caller-thread completion joins the worker.

## Consequence

Validate at the point work is admitted and again after a scheduling delay. Keep cancellation/revocation/result delivery linearised. Uncooperative workers remain counted after the caller's deadline until they actually exit; reaching a timeout is not child-process termination.

## Follow-up

T012/T014/F08 must use the same guard, check cancellation while performing I/O, terminate native children/channels and hold the native lifecycle lease for the actual executor lifetime. Actual SSH/symlink/phone/mutation proof remains open.
