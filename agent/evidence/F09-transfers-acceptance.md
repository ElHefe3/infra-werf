# T017 desktop streamed transfers

Date: 2026-10-26
Revision: d6da833 + reviewed T017 working tree; completion commit includes this record.
Result: pass

## Acceptance mapping

- Empty, 713-byte and 8 MiB uploads/downloads pass actual system OpenSSH/SFTP with matching SHA-256. Production buffers are bounded to 32 KiB chunks / 1 MiB frames; the fixture observed max_chunk=32768. No file payload enters renderer, SQLite, audit or evidence.
- Actual queue/cancel, final-STAT cancellation, background view release, collisions, concurrent local destination creation, corrupt readback, interrupted write/fresh retry, lost publication acknowledgement, absolute expiry and local-temp cleanup pass. No automatic replay or overwrite is implemented; publication failures can remain uncertain and require inspection. Interrupted uploads retain only an owned restricted staging location when cleanup cannot be confirmed.
- Native Windows open/save dialogs authorise opaque session/host-bound ten-minute local grants. Both real picker/review/transfer journeys passed through the native WebView/Rust/OpenSSH app. An initial review really expired during unavailable approval review; no download started from the expired grant. A new session and native selection were required.
- Final native fixture: `.tools/native-transfer-proof-8a5e156e-1c8e-4776-8065-727a9849da7e/`. Both 8 MiB files hash to `2a8acb66aa869b29f77fa6b646b3cf1e800a1546c876ce68f9d79184c29d7264`. Actual upload continued in Transfers after leaving Files; completion closed the hidden session (active=0) and cleared names/paths/checksum. Alt-F4 was blocked with the authenticated session open, then closed normally after cleanup. Native inspection used the Computer Use skill and `cua_repl`; the helper only prepares private synthetic records/files.

## Verification and review

- `mise run test:files`: actual fixture integration passes, including existing browse/mutation regressions and new transfer cases. Ignored `.tools/transfer-core-proof.log` records the run.
- `mise run test:feature -- T017`: 41 ordinary Rust tests and 4 adapter/model tests pass; generated bindings, TypeScript and tracker fixtures pass. `.tools/transfer-regression.log`.
- `mise run smoke:desktop`: actual debug application built. `.tools/transfer-native-build.log`. Native picker and transfer proof used the reviewed core; final frontend-only long-verification timer refinement passed type/static checks at closure.
- Final existing native browse/stat/lock/trust/server-edit/disconnect/create/rename/move/delete regression passed in `.tools/native-files-proof-2d1d0718-e3f5-4bf3-b4d6-a1337efffc05/result.json`, including unchanged fixture trust and map, eleven confirmed mutation requests and no page errors. A first 5s release wait timed out under load; instrumented rerun passed without a product change. The fixture now allows a bounded15s cleanup window. A separate real release-with-no-transfers test passes. The deliberate lost-ACK fixture deadline changed1s→3s so Windows process startup is not its assertion; stalled ACK remains60s. No production deadline changed.
- Independent review found a cancellation gap at publication, ignored cleanup I/O failures and a late-checksum redaction race. Publication now gates cancellation under the record lock; failed cleanup invalidates the channel; each metadata snapshot redacts and clones atomically. Deterministic regressions pass. Reviewer confirmed all three resolved, with no remaining findings.

## Environment, repairs and limits

Pinned Node 26.12.1, pnpm 12.15.0, Rust 1.99.1 MSVC, Windows/OpenSSH/WebView2, Tauri 2.12.5. Native dialog=2.8.3, SHA256=0.10.9; no frontend FS/dialog permissions. All hosts/files/keys are synthetic loopback fixtures; owner infrastructure was untouched. Original Python database SHA256 remains `2CD22BD9AFBE8BF95481C5FA2D33A9A4773BEF6743D278875FB2CB68316260CE`.

One regression build exhausted disk; removed only the verified workspace `target/debug/incremental` generated cache (6,986,479,972 bytes). Rebuild/regressions then passed. UI test-helper startup readiness was corrected. Native approval review temporarily hit its usage quota; retry happened only after the owner requested continuation. No approval controls were bypassed.

T017 is ordinary SFTP transfers only. T018 reviewed elevated placement, terminal, sudo, health and release/owner acceptance remain separate desktop tickets. Mobile is future S09. Representative 8 MiB proof establishes streaming behavior, not a broad performance benchmark or exhaustive 32 GiB throughput claim. Jobs/history are bounded and in memory; restarting never resumes a mutation automatically. Rollback: revert this feature commit; original/native databases are unchanged by transfers.
