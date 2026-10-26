# D027 — Protected staging must outlive its view only for cleanup

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T018, T026

## Finding

A protected upload creates ordinary-user plaintext in a private SSH staging directory before any sudo operation. The directory is mode 0700 and the file is mode 0600, owned by the authenticated SSH user. Locking the Files view cannot revoke the SFTP channel while staging is being admitted, while bytes are in flight, or while a completed stage still awaits UI acknowledgement or cleanup.

Use one per-session lifecycle gate to serialize stage admission with lock/revocation. A hidden retained stage may use its backend-held SFTP handle for cleanup after the five-minute idle view expires, but only until the existing 30-minute absolute session deadline. The reaper retries cleanup on idle expiry. Absolute expiry closes the SSH session even if cleanup fails; the restricted stage path remains available for recovery through a new Files session or the user's normal SSH account.

## Evidence

On 2026-10-26, the ignored `atlas-core` elevation fixture exercised stage-then-lock-before-ack, duplicate cleanup requests, idle-expiry cleanup, absolute-expiry disposal, exact owner/mode checks, and in-flight release. It passed with the disposable loopback Linux SSH/sudo container. Independent bounded review approved the `stage_gate` lifecycle synchronization and expiry behavior. The Windows debug smoke build and native fresh-sudo regression passed in the same working tree.

## Consequence

Do not close/revoke a still-valid session merely because the Files component unmounted while a private stage is pending. Do not extend the interactive session past its absolute deadline to preserve cleanup. If the stage survives that deadline, report its exact restricted location; a new ordinary SFTP session can inspect and remove it without sudo.

## Follow-up

T026 should retain proof that absolute-expiry fallback leaves only owner-restricted data and that the recovery location is actionable. A force-quit or process crash can discard the in-memory UI reminder, so startup discovery/remediation of abandoned `.atlas-protected-*` directories remains a release-recovery consideration.
