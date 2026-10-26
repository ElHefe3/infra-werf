# S04 — Files and transfers

Date: 2026-10-26

Revision: 7c2e941 + reviewed T018 working tree (uncommitted)

Result: pass

## Scope

S04 covers T015–T018 on Windows desktop. All remote effects were exercised against disposable loopback SSH/SFTP/sudo fixtures; local selections and databases were synthetic. Owner hosts, remote files, trust/configuration and the reference SQLite database were not modified.

## Acceptance mapping

- **Navigation, metadata and deliberate changes:** [F06–F08 acceptance](F06-F08-desktop-acceptance.md) and [T016 acceptance](F08-mutations-acceptance.md) cover navigation, permissions/stat, unusual and non-UTF-8 names, links, mkdir/rename/move/delete, collision/denial, confirmed previews and native Windows journey.
- **Native transfers and integrity:** [T017 acceptance](F09-transfers-acceptance.md) covers Windows native picker grants, actual SFTP upload/download round trips and matching SHA-256, progress, cancellation, retry, session release and close guard.
- **Interruption, expiry and no silent replay:** T017's fixture covers queued/background transfer cancellation, uncertain publication acknowledgements, retry and session expiry. [T018 acceptance](T018-acceptance.md) adds lock during/after staging, idle cleanup, absolute-expiry session disposal and private 0700/0600 fallback.
- **Protected placement:** T018 proves unprivileged SFTP staging, explicit destination/owner/group/mode review, a separate fresh sudo action, wrong-password/cancel/replay/collision rejection, exact content/ownership/mode, and cleanup in the real SSH/sudo fixture.

## Verification and limits

The native Windows T017 picker/transfer journey and T016 file-change journey passed. The Windows debug build and T013 native elevation journey passed with T018 included. T018's protected stage-and-apply sequence was exercised through the actual Rust/OpenSSH/SFTP/sudo fixture rather than a combined native file-picker-to-placement journey; the picker and native elevation flows have separate native proof. See linked ticket evidence for commands, artifacts, reviews, failures and limits.

Phone/coordinator and physical-device acceptance remain future S09. This scope evidence does not claim an installer, real-host writes, owner release acceptance, or startup discovery after a force-quit during unresolved staging. [D027](../discovery/D027-protected-stage-session-lifecycle.md) records the recovery constraint for T026.
