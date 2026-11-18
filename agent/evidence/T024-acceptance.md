# T024 acceptance — Integrated desktop recovery

Date: 2026-10-26
Revision: application source at `7812aae` plus the T024 verification-harness changes in this working tree; no application Rust or UI source changed.
Result: pass
Feature/ticket: F12 / T024

## Environment and target

- Windows x64, native Tauri 2.12.5 / WebView2, Rust 1.99.1 MSVC, Node 26.12.1, Python 3.12.
- All migration, restart, and database recovery cases used generated synthetic fixtures. The native browser-debugging endpoint was loopback-only.
- No user database, owner SSH configuration, real host, or server was read or changed. Server 1 remains corrupted, offline, and powered off.

## Acceptance mapping

| T024 criterion | Evidence | Result |
| --- | --- | --- |
| Core feature acceptance and shared contract/storage/auth regression are valid | Prior accepted evidence remains valid for T018, T020–T023 and the earlier S01–S04 tickets. `cargo test -p atlas-core --test operations --test contracts --test store --locked --offline -- --test-threads=1` passed **24 tests** (13 operation, 5 contract and 6 store), covering session identity/revocation/expiry, protected result redaction, strict contracts, SQLite ownership and migration recovery. | Pass |
| Migration, restart and rollback preserve identity/layout/relationships and approved data; no concurrent legacy/replacement writers | `python scripts/verify_data.py` passed against its generated 2-server synthetic legacy database: 1 directed edge, 5 events, baseline parity, native report import, rollback parity, and rejection of a concurrent Python writer. The actual Tauri/WebView2 journey passed close/relaunch persistence for edited servers and edges, visible incompatible-database retry, and byte-preservation of the incompatible file and synthetic source. | Pass |
| Failure, revocation and expiry across file, terminal and service flows leave no credential or protected-result persistence | Reused T018's actual SFTP/sudo denial, cancellation, Lock, cleanup and idle/absolute-expiry proof; T021's native terminal Lock/stale-input/database checks; T022's protected inspection/log non-persistence; and T023's session expiry/revocation, uncertain-result and credential non-persistence proof. Shared operation tests also passed mid-execution revocation and expired-session denial. | Pass |

## Verification

- Native Windows journey: sanitized result `.tools/desktop-proof-9d68a37a-02a2-4bcb-b081-c7f12de5bd03/result.json` (local artifact, removed 2026-11-18; the summary below is the retained record). All nine result groups passed, including 1280×720 and 1440×900 desktop layouts, 540-service search, keyboard map interaction, server/edge edits, second-instance handling, native close/relaunch, workspace tabs and startup error recovery. It reports zero page errors and `legacyUnchanged: true`.
- Migration proof files are retained under `.tools/data-proof-6b2520955bab44af944bf5e0d85982db/`; generated fixture only.
- `cargo test -p atlas-core --test operations --test contracts --test store --locked --offline -- --test-threads=1`: **24 passed, 0 failed**.
- T023 evidence documents the broader Rust full-suite limitation: the unfiltered suite encountered an output-file linker lock, then an unrelated health cancellation failure and two local health tests exceeding 60 seconds. No full-suite pass is claimed here.
- `scripts/verify_data.py` parity comparator and Windows writer-lock harness refinements are documented in [D036](../discovery/D036-parity-checker-provenance-fields.md) and [D037](../discovery/D037-windows-asyncio-child-process-fixture.md). The native harness's successful Windows launcher handoff and refreshed terminal assertion are documented in [D038](../discovery/D038-windows-tauri-launcher-handoff.md) and [D039](../discovery/D039-terminal-locked-state-assertion.md).

## Limits and recovery

Proof used synthetic local state and does not accept the Windows release, perform owner cutover, or establish real-host compatibility. The Python reference remains intact; rollback from the synthetic migration restored the original IDs, layout, relationships, approved inventory and events. The native restart check confirms that a malformed replacement database is retained and surfaced for retry rather than overwritten. Native terminal absolute-expiry was not time-accelerated in this UI journey; terminal Lock and the shared session cleanup path were exercised, while idle/absolute file-session expiry is covered by T018's fixture.

No fixture container was created by T024. Synthetic proof directories are ignored under `.tools/` and retained for traceability.
