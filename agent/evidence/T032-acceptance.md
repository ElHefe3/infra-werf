# T032 desktop operation guard acceptance

Date: 2026-10-25
Revision: working tree based on **e6760a3**; final local completion commit recorded in Git/checkpoint.
Result: pass
Ticket: **T032**, F04/S02 desktop-only shared operation foundation, extracted from T014 to respect desktop-first priority. T014 and actual SSH/phone integration remain open.

## Environment and fixture

Pinned mise Rust 1.99.1 MSVC, Node 26.12.1/pnpm 12.15.0, ts-rs 12.0.1/serde 1.0.229 on Windows x64. No new dependencies or tool upgrades. In-memory synthetic authorities, sessions, targets, scoped paths, workload IDs and controlled executors; no real credentials, host connections or data changes.

## Proof collected

- Final `cargo test -p atlas-core --locked`: **27 core tests passed**, including 11 guard tests (2 deterministic race regressions + 9 integration cases) and all 16 storage/contracts/inventory regressions. Private command proof: `.tools/logs/t032-final-core.log`. `mise run test:feature -- T032` also passed its focused tests, generated contract comparison, TypeScript and tracker self-tests.
- Caller/session/server/host binding, foreign broker/caller denial, missing authority/session, session expiry and prepared/mid-run revocation pass.
- Unknown/extra shell, credential, mutation and claimed-authority fields fail closed. Bounded decoder returns fixed errors without the synthetic secret value. Scoped literal path handling, traversal/sibling-prefix denial and canonical-resolution rejection pass; authenticated workload membership is required.
- Tickets execute once, including concurrent replay. Prepared/running cancellation cannot restart the worker. Partial/oversized output is discarded even when the executor ignores the sink's error. Raw returned executor errors are redacted; bounded audit has no payload field.
- An uncooperative read-only callback cannot block return past the ticket deadline. It remains counted until actually released. Four concurrent workers saturate the budget; the fifth is denied and capacity recovers after actual completion.
- Generated TypeScript adds only request/ticket schemas and fixed error codes. Authority/session grants have no wire constructor. No SQLite schema, native IPC, UI or capability changes.

## Repairs and limits

The first decoder negative test exposed Serde's unit-variant extra-field acceptance. Replacing the zero-field unit variant with an empty struct variant made unknown fields fail as required. Result delivery, cancellation and revocation now share a registry lock at final delivery. Worker spawn failure also releases its resource count and fails closed.

This is controlled library proof, not a native SSH success. The guard does not authenticate, kill OS children, implement SFTP canonicalization/symlink race safety, erase frontend protected views or override global executor panic/log hooks. These explicit integration obligations are in [OPERATIONS.md](../OPERATIONS.md) and T012/T014/F08. No mutations are enabled; future mutation idempotency remains T014/F08/F09. The existing desktop window/binary is unchanged and its prior acceptance is reused.

## Final checks and review

`mise run check` passed generated-binding comparison, TypeScript, Rust fmt and Windows workspace compilation. `mise run check:android-core` passed on the configured NDK/ARM64 target without UAC/APK rebuilding. Known ts-rs attribute warnings are retained; Rust negative tests and generated TS prove the actual decoder/type shapes.

Independent bounded review found a P2 stale-admission race and a P3 premature worker-recovery assertion. Admission now revalidates while holding registry→phase locks, and delayed worker entry checks again before the callback. Two deterministic tests cover an already validated ticket cancelled/revoked before admission and a revoked context before worker entry. Worker recovery waits for bounded actual cleanup. The reviewer confirmed both findings resolved, with no remaining actionable issues in the reviewed slice. Final platform checks above passed after the fixes.

No owner/device acceptance, production SSH success or UI change is claimed. The desktop map remains the previously accepted build. No real database, credential, known-host entry or infrastructure was changed.
