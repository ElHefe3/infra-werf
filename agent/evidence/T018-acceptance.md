# T018 — Reviewed protected placement

Date: 2026-10-26

Revision: 7c2e941 + reviewed T018 working tree (uncommitted)

Result: pass

## Included slice and environment

Ticket: T018 in F09/S04. Windows x64, Tauri 2.12.5, Node 26.12.1, pnpm 12.15.0 and Rust 1.99.1 MSVC. All SSH, sudo, files and destination paths were synthetic in a disposable local Linux container reached by system OpenSSH/SFTP. The native Windows elevation regression used a separate synthetic fixture and temporary database. No owner host, map database, SSH configuration or trust file was changed.

## Acceptance mapping

- **Unprivileged staging and reviewed elevation are separate:** The real SFTP fixture staged content as the fixture SSH user in a verified 0700 directory with a 0600 file. Readback SHA-256 was checked before protected placement. A separate preview named the exact destination, owner, group and mode; only a fresh sudo action applied it. The UI exposes this sequence as separate pick, stage, review and confirm steps. The existing native elevation journey separately passed in the Windows app.
- **Denial and collision preserve the protected destination:** Wrong sudo password, cancelled preview and replayed operation were rejected without applying the destination. A pre-existing collision retained its sentinel bytes. Bidi-control destination text was rejected. The isolated fixture also verified a Lock after stage completion but before the frontend acknowledgement; the cleanup handle remained usable and removed the private stage.
- **Successful application and cleanup:** The fixture compared exact content, UID, GID, mode and SHA-256 after publication; the stage was removed. A duplicate cleanup returned `not_found` without closing the still-open user Files session. Cancellation during an in-flight upload removed partial private staging. Idle expiry retried cleanup successfully. At absolute expiry the SSH session was disposed, and the remaining synthetic file stayed mode 0600 inside its owner-only mode 0700 directory; the UI reports the exact location for ordinary-user recovery.

## Executed proof

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --locked`: passed. Existing `ts-rs` serde-attribute warnings remain.
- `pnpm check` with the pinned Node/pnpm: passed.
- `cargo run -p atlas-core --bin export-contracts --locked -- --check`: generated TypeScript contracts matched Rust definitions.
- `cargo test -p atlas-core --test elevation --locked -- --ignored --nocapture`: **1 actual SSH/sudo fixture passed**. Output covered banner spoof protection, fresh sudo, staging permissions/hash, lock-after-ack cleanup, idle and absolute expiry, duplicate cleanup, in-flight cancellation/partial cleanup, cancellation, wrong password, exact metadata, no-overwrite collision, host/config/key rotation and redacted audit. Fixture shutdown cleaned its temporary container and child/lease resources.
- `pnpm tauri build --debug --no-bundle -- --locked`: passed; built `target/debug/atlas-native.exe`.
- `scripts/verify_native_elevation.mjs`: native WebView2/Rust/SSH fixture passed review cancellation, wrong-password clearing, successful fresh sudo, repeat challenge, Lock and database/config/trust preservation; zero page errors.
- Independent bounded review: **Pass**, including stage admission/close synchronization, cleanup after idle expiry and absolute-expiry disposal.

## Refinement and limits

The first test run exposed only a fixture-harness leak: the absolute-expiry case intentionally left a restricted synthetic stage that later confused a test-wide “no stages remain” wait. The test now removes that exact file and directory after checking their permissions; the final run passes. Three review findings were addressed: bidi controls in destination previews, session release during staging, and the stage-start/Lock race. No production scope or permission policy was relaxed.

The complete protected-placement UI flow did not use a separate native file-picker journey. The local picker/opaque grant has native Windows proof in T017, while T018's actual protected stage/apply path is covered by the real SFTP/sudo fixture and the shared native elevation flow by T013. A force-quit can discard the in-memory reminder for a restricted stage; startup orphan discovery is recorded in [D027](../discovery/D027-protected-stage-session-lifecycle.md) for release recovery review. No installer, real protected destination, release signing or owner acceptance is claimed.

## Cleanup and rollback

The disposable Linux fixture shut down after the test. The native harness used a temporary database and fixture SSH config/trust and reports no page errors; these remain under ignored `.tools/native-elevation-proof-b5887a7e-43a6-4e71-91ee-c06d5c626a8b/`. No changes were made to real remote files. Rollback is to revert the T018 working-tree changes; no user data migration or real-host rollback is required.
