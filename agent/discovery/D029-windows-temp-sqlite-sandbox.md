# D029 — Windows sandbox restrictions affect temporary SQLite integration tests

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T020

## Finding

The inventory integration tests use isolated temporary SQLite databases. In this Windows workspace, the restricted default shell reported SQLite `Conflict` errors while creating or writing those temporary test databases. Running the same focused Cargo test with process-level escalation passed without source changes. The evidence points to sandbox access on the temporary test files, rather than an application-level conflict; it does not establish the exact filesystem operation denied by the sandbox.

## Evidence

On 2026-10-26, `cargo test -p atlas-core --test inventory --locked` failed in the restricted invocation with temporary-database `Conflict` errors. The same command, run with explicit process-level escalation and unchanged code, passed all five tests. The test data remained in temporary directories; no owner database was used. The native proof `.tools/native-inventory-proof-62774b3f-ac81-47aa-8197-87c3c0057d82/result.json` also passed against an isolated loopback SSH fixture.

## Consequence

Treat this as a test-environment boundary, not a reason to weaken the project or global sandbox. If the restricted run fails with the same temporary SQLite symptom, rerun only the focused temporary-database test target with the narrow process approval. Do not elevate app execution or use an owner database as a workaround.

## Follow-up

Keep T020 evidence explicit about the temporary test environment and fixture-only data. Revisit only if an equivalent least-privilege test setup can avoid the sandbox limitation.
