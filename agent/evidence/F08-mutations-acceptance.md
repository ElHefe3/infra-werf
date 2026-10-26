# T016 — Deliberate desktop file mutations

Date: 2026-10-26

Revision: 14ea114 + reviewed T016 working tree; the completion commit contains this record.

Result: pass

## Included slice and environment

The owner requested the next desktop slice after T014/T015. T016 adds native create-folder, rename/move and confirmed deletion through the existing protected SFTP session. Phone confirmation/navigation/error proof remains explicitly T009/T010 and the S04 release gate. Recursive deletion, overwrite, transfers and sudo are unavailable.

The proof uses Windows x64 Tauri/WebView2, actual Rust IPC and system OpenSSH/SFTP. Existing mise pins and fixture dependencies are unchanged: Rust/Cargo 1.99.1 MSVC, Node 26.12.1, pnpm 12.15.0, Python 3.15.1; Tauri Rust 2.12.5 and React 19.3.1. All writes are restricted to the private loopback fixture's write-area. No real remote files, identities, trust or owner inventory are test targets.

## Acceptance mapping

- **Create/rename/move and failures:** Actual SFTP and native journeys create Unicode folders, rename a space-containing file, pin a source while navigating to a destination, and verify moved content. Collision checks run before preview and confirmation; an injected server-side destination collision preserves both the source and existing destination. Permission failures, stale source metadata, invalid components, raw-byte names and literal shell-looking names use typed packets rather than a shell. Views refresh actual results; no optimistic deletion/rename is shown.
- **Deliberate deletion/cancellation:** A fresh, expiring, single-use preview names the server/resolved user endpoint and exact source/destination. Confirmation is separate from preview. Cancel discards the intent without a write. File/link deletion uses REMOVE; directory deletion uses RMDIR and rejects nonempty folders. Native proof confirms link-target preservation, empty-folder deletion and non-recursion. Recursive deletion is explicitly unavailable. Literal backslashes are escaped so control/raw-byte representations cannot collide with real escape-text filenames; native proof deletes the raw-byte entry while preserving its distinct literal counterpart.
- **Session/target/actual outcome:** Wrong host, forged resource, unknown fields, stale/cancelled/locked intents and dispatcher substitution are denied. The existing broker still enforces authority, revocation, deadline and single-use tickets. Mutation plans cannot execute through read/bootstrap dispatchers. Metadata/type/canonical parent and destination absence are rechecked immediately before the fixed write request. Tabs/dialog exit remain blocked through acknowledgement/reconciliation; explicit Lock can interrupt and reports uncertainty. An applied-but-unacknowledged mutation closes the session, reports OutcomeUnknown and never retries. One atomic state publishes sent/rejected/applied acknowledgement, preventing a success-publication race during timeout/revocation.

## Executed proof

- `mise run test:feature -- T016`: **40 ordinary Rust tests** (38 core + 2 native), **4 client/model tests**, generated bindings/types and tracker checks passed. Added focused name/component and mutation-dispatch substitution/replay cases; existing authority/lifetime/storage tests remain passing.
- `mise run test:files`: **1 explicit real-system SFTP integration test passed**, reusing the existing fixture for both T014/T015 lifecycle and T016 mutations. It covers preview/cancel, create/rename/move/delete, non-UTF-8 source bytes, symlink identity, collision/stale source/destination/denial/non-recursion, locking, expiry/revocation and a mutation applied before its acknowledgement is withheld. Actual child/lease cleanup and no replay are asserted.
- `mise run test:file-changes-native`: **pass**, including the complete prior browse/lock/trust/cancel/invalidation journey and new mutation controls. Final private proof: `.tools/native-files-proof-9d6dbaaf-ba10-4b39-9ce4-2898613699d7/`. It contains result metadata, delete-preview/reconciliation/uncertain-outcome screenshots and zero page errors. Eleven counted fixture mutation requests include the expected nonempty-directory rejection; they are not eleven successful changes. Config and both trust-file hashes stay unchanged. Map/database bytes remain unchanged except the harness's two explicit synthetic alias edits used to test backend invalidation.
- `mise run smoke:desktop`, `mise run check`, `mise run check:android-core`: pass. Native execution remains Windows-only; the shared core/contracts compile for ARM64 Android. Only existing ts-rs attribute warnings remain. Final cosmetic refinement gives destructive confirmation a white-on-red button; the delivered artifact's complete native journey passed again.
- Tracker proof remains **14 rejection + 2 positive completion/phased-entry fixtures**, with default full-scope gates preserved. No new runtime dependency or tool upgrade was needed.

The previous native map editing/restart and SSH Settings proofs remain valid: their implementation is unchanged. The combined current native Files journey re-exercises the affected session/guard and workspace boundaries. No APK, installer, physical phone or owner release acceptance is claimed.

## Failures, review and refinement

The Windows fixture initially confused a missing non-UTF-8 virtual destination with a Windows filesystem lookup, then handled its known read-only virtual name after the absence fallback. Both fixture issues were corrected and the actual SFTP proof passed. Byte-keyed virtual entries prove protocol preservation; this does not claim Windows can store arbitrary POSIX bytes. [D019](../discovery/D019-sftp-mutation-outcomes.md) records the facts and primary protocol/source references.

The first native regression observed a remaining connection after leaving Files. Protected lock/cancel now runs before optional listener/state teardown, and the final combined journey proves cleanup. Independent review found that leaving the component during an in-flight mutation could discard the uncertainty warning, and that escape-text filename labels could be ambiguous. Parent busy admission and injective backslash/byte/control display resolved those findings; review confirmed no remaining blockers. A separate atomic-publication gap was removed before final verification. Native delayed-ack and colliding-name cases exercise the repairs.

## Limits and recovery

Standard SFTP rename is requested without an overwrite-capable extension; existing destinations are checked twice and the tested server rejects them. This does not claim atomic inode identity or race-free comparison against all external writers/server filesystem fallbacks. Source scope remains the SSH account's filesystem, not a restricted-root jail. Preview source metadata is a freshness check, not a cryptographic content identity.

After uncertain delivery, reconnect and inspect before preparing a new change. Never replay the consumed ticket or resume under expired credentials. Operation/path previews and receipts remain ephemeral; audit/public inventory contain no protected payload. Tests preserve fixture source/target sentinel bytes where required and shut down their own processes/listeners. Owner data and real hosts were not modified. The reference DB SHA256 remains `2CD22BD9AFBE8BF95481C5FA2D33A9A4773BEF6743D278875FB2CB68316260CE`. Rollback is the previous native build with preserved baseline data; no real remote rollback is needed.

Normal Atlas was reopened: one native process/window, zero temporary CDP listeners and the same reference digest. The delivered executable SHA256 is `8E340F2E8073F87923B68D763A0F12993C1E038211FDC5149B8FD14762CDEDEA`. No owner/physical-phone/release acceptance is inferred.
