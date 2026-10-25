# F04 Windows shell acceptance (T008 slice)

Date: 2026-10-25
Revision: desktop working tree based on **0145d7a**, same native build as [F03 proof](F03-acceptance.md).
Result: pass
Ticket: **T008** Windows local workspace lifecycle. T009 and the overall F04/S02 gates remain open.

## Actual proof

`mise run smoke:desktop`, `mise run test:desktop` and `cargo test --workspace --locked` passed on Windows x64. See F03 for exact tools/private artifacts.

- Real persisted state and typed Rust map mutations run inside Tauri, without Python or a frontend dev server at runtime. Native store uses its own app-data file; first import creates separate native/recovery files through the F02 read-only migration.
- Second process exits successfully; single-instance callback restores/focuses the existing main window. Native Quit follows Tauri's close event; relaunch preserves all fixture changes.
- Shared admission lock serialises close with operations. An active operation prevents close; after completion close is allowed and further admission is rejected. Unit test covers this race boundary plus actual store reopen/persistence.
- Missing migration source remains a visible failed state with retry, never silently creates empty replacement data. Native UI corrupt-database test confirms visible error/retry and byte-for-byte preservation.
- Window capability allows only core window/event operations and explicit close. No shell/filesystem plugin, arbitrary shell command or network listener is added. Map IPC invokes core validation. Remote coordinator explicitly shows unavailable until F05.

## Staged lifecycle boundary

Only local domain work exists in this slice. Phone sessions and transfers are unavailable; no claim is made that their real journeys were tested. F05/F08/F09 must admit their work through the same lifecycle guard before enabling it, and verify close/quit warnings with actual active remote work. This obligation remains core acceptance, not a deferred feature or a silent close shortcut.

## Limits and recovery

Independent bounded review found no additional original-data/IPC/capability/lifecycle correctness issues. Its two UI keyboard findings were fixed, confirmed resolved and passed the final native regression; see F03. Installer/release/owner acceptance are T024–T026. Android adapters are T009. Private synthetic proof files remain ignored. Closing Atlas releases the native SQLite owner lock; the untouched Python reference and private recovery remain available. No automatic live cutover or cross-application sync.
