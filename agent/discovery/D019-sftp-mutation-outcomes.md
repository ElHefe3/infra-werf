# D019 — SFTP mutation semantics and fixture encoding

Date: 2026-10-26

State: verified

Affected: T016

## Finding

SFTP v3 separates a standard rename that rejects an existing destination from the overwrite-capable POSIX rename extension. The [v3 protocol description](https://datatracker.ietf.org/doc/html/draft-ietf-secsh-filexfer-02#section-6.5) specifies the former. [OpenSSH 9.5 source](https://github.com/openssh/openssh-portable/blob/V_9_5_P1/sftp-server.c#L1202) also exposes server/filesystem-dependent race-prone fallbacks. Atlas requests standard rename, checks absence before preview and commit, and never requests overwrite. This is not a guarantee of atomic comparison against every other remote writer.

A missing acknowledgement is independent from whether a write was applied. A timed-out/disconnected confirmed mutation closes its session, reports an uncertain outcome and requires inspection before any new attempt. One atomic transition publishes rejected/applied acknowledgement; separate flags could briefly misclassify successful-but-undelivered work. It cannot silently retry the same write.

## Evidence

The initial real SFTP mutation test reached a non-UTF-8 move preview and returned fixed IoFailure. Its fixture was passing the missing destination to Windows' Unicode filesystem API. The fixture now simulates absent raw-byte names as SFTP NoSuchFile and keeps virtual renames in byte-keyed state. A subsequent run exposed lookup precedence: that fallback also hid the earlier read-only fixture's known raw-byte name. Known virtual entries now precede the absent-name fallback. Corrected real SFTP and native journeys passed; the application preserves wire bytes.

Review found that escaped invalid/control bytes and literal backslashes could produce identical labels. Display now escapes literal backslashes too. Unit and native cases distinguish both filenames and delete one while preserving the other. During confirmed writes the parent admission guard disables tab/dialog exit; explicit Lock reports uncertainty. Protected cleanup precedes optional event teardown. The final native proof includes tab/close admission, backend invalidation and no automatic retry after an applied-but-unacknowledged change.

## Consequence

Preserve raw source bytes across browser navigation with one session-bound pinned resource. New names are literal bounded components. Preview and confirmation use a single-use intent, fresh metadata/type/parent checks and the existing admission guard. Delete uses REMOVE for links/files and RMDIR for empty directories; no recursive traversal is offered.

## Follow-up

Independent review confirmed the repairs with no remaining findings. Keep physical-phone confirmation and coordinator proof in T009/T010. Preserve the distinction between protocol-byte proof and Windows fixture filesystem simulation; do not upgrade standard SFTP semantics into an atomic-inode or restricted-root claim.
