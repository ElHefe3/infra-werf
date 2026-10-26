# Atlas restart checkpoint

Date: 2026-10-26
State: S01/S02 desktop complete. T017 transfers now passes; S03–S06 desktop remains authorised. Mobile/coordinator/Android is future S09. Continue all desktop batches without asking routine permission between them.
Revision: d6da833 + reviewed T017 working tree; its completion commit includes this checkpoint.

## Resume

Read [AGENT.md](../AGENT.md), the selected packet and its evidence only. Next: T012 existing SSH identities/agent and host trust, then T013 fresh per-action sudo. After that T019/T020 monitoring, T021 real terminal, T022/T023 services, T018 elevated placement, and T024–T026 Windows release/actual owner acceptance. Do not infer release acceptance or mobile completion from desktop source or fixture tests.

T012's configured-key, strict trust, unknown/changed/cert/alias/fallback/authentication/argument handling is already proven by T033/T014. Reuse that evidence; add actual Windows agent-backed signing proof without moving owner keys or changing ssh-agent service/trust. T013 still requires fresh deliberate sudo; protected session lifecycle already exists. Pairing/device revoke stays S09.

## Delivered

- [F01](F01-acceptance.md): mise-pinned Windows/native foundations and historical ARM64 foundation APK. Mobile packaging/physical acceptance is future S09.
- [F02](F02-acceptance.md): Rust/generated TS, native-owned SQLite, safe-copy migration and rollback, writer guards, classified imports and stable IDs; resolved independent review.
- [F03](F03-acceptance.md)/[T008](F04-desktop-acceptance.md): native map, directed links, saved full inventory, keyboard/pan/zoom/layout/restart/error/single instance. Real owner native copy has 4 servers/540 workloads; technical proof only.
- [T032](T032-acceptance.md)/[T033](T033-acceptance.md): backend authority/admission/output/cancel and strict real OpenSSH verification. No key/password transport, automatic trust change, forwarding or helpers.
- [T014/T015](F06-F08-desktop-acceptance.md): real protected SFTP, paging/stat/opaque handles, 5min idle/30min absolute, Lock/expiry/revoke/disconnect clearing and actual child/lease cleanup. Paired transport remains S09.
- [T016](F08-mutations-acceptance.md): native confirmed mkdir/rename/move/delete, pinned move source, no overwrite/recursion, non-UTF8 escape identity, safe rejection and uncertain ACK. Resolved review.
- [T017](F09-transfers-acceptance.md): native open/save selection, opaque ten-minute grants, streamed SHA256 upload/download, bounded queue/progress/cancel/new attempts, private staging/no-clobber, background Files release and close guard. 41 ordinary Rust + actual SFTP integration + 4 client tests; native picker/upload/download/background/clearing and native browse/mutation regression pass. All three review findings resolved. See [D021](../discovery/D021-transfer-publication-and-build-cache.md).

Final transfer fixture: `.tools/native-transfer-proof-8a5e156e-1c8e-4776-8065-727a9849da7e/`; 8MiB native upload/download SHA256 `2a8acb66aa869b29f77fa6b646b3cf1e800a1546c876ce68f9d79184c29d7264`. Final old native journey `.tools/native-files-proof-2d1d0718-e3f5-4bf3-b4d6-a1337efffc05/result.json`. Source/fixture logs are ignored, not tracked. No real host files, trust, passwords or keys were modified.

## Data and tools

Original Python `data/atlas.sqlite3` SHA256 remains `2CD22BD9AFBE8BF95481C5FA2D33A9A4773BEF6743D278875FB2CB68316260CE`. Preserve it. Native owner DB is separate appdata `dev.atlas.workspace`; never run concurrent Python/Rust writers against the reference. Server01 is offline/corrupt per owner; 02/03/04 configured addresses/SSH work. Preserve their addressing/trust.

Pins: Node26.12.1, pnpm12.15.0, Rust1.99.1 MSVC, Python3.15.1, mise2026.9.1; Tauri2.12.5/CLI2.12.4, ts-rs12.0.1, SQLite/rusqlite0.41.0, tempfile3.27.0, dialog2.8.3, sha2 0.10.9. Do not upgrade mid-feature. Android's established tools remain preserved for S09.

Use `mise run exec -- ...` for child PATH. Full path: `C:/Users/owner/AppData/Local/Microsoft/WinGet/Links/mise.exe`. Normal user build/cache access often needs tool escalation; never weaken sandbox/global policy. Close the exact owner Atlas process normally before rebuild; active work must prevent closing. Background helpers hidden; owner app visible. No push/deploy requested.

Verified tasks: `test:feature -- T017`, `test:files`, `test:file-changes-native`, `check`, `smoke:desktop`, `start:desktop`. Use existing test fixtures/caches. Windows disk space is tight: generated `target/debug/incremental` reached6.99GB; its verified deletion freed space after disk112. Do not delete user data. Synthetic process startup/cleanup tests now allow3s/15s respectively while production budgets stay unchanged.

Computer Use via `cua_repl` works for actual OS file pickers; persistent node_repl still fails kernel asset initialization. Existing CLI native regression harness uses isolated WebView2 CDP; disable debug listeners in the normal owner launcher. A temporary automatic approval usage quota interrupted one native download; genuine expiry cleared its pending grant. Owner then requested continuation; no approval bypass occurred.

Tracker:33 tickets/15 features/9 scopes;25 desktop core/8 deferred. Done14, ready1(T012), todo10, deferred8. Feature packet metadata is authoritative. Full-scope gate rules remain intact. Deferred scopes are not substitute work for desktop blockers.
