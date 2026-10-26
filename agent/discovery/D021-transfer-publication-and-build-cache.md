# D021 — Streaming requires a separate publication and cleanup boundary

Date: 2026-10-26 · T017

Byte progress reaching 100% is not completion. Uploads close and read back the private payload, compare SHA-256, then publish without overwrite. Downloads sync/read back the private local temp before no-clobber publication. The final cancellation gate must follow every remote preflight and share a lock with Cancel; once publication starts, cancellation cannot promise to prevent it. Lost publication acknowledgement is uncertain, never an automatic retry.

The broker can return cancellation before its native callback exits. Queue completion therefore waits for actual callback cleanup; weak history references must not keep native sessions/leases alive. Cleanup transport errors invalidate the SFTP stream even if the original user result was Cancelled. A response with a delayed old request ID must never be consumed by another operation.

Redacting a record and cloning it later under a second lock permits a late callback to restore a checksum after Lock. Redact and clone each response under the same record lock. The renderer also discards stale poll generations and clears details on session closure.

Leaving Files releases visible resource capabilities; approved jobs retain immutable queued capabilities and close the hidden session when the final worker exits. Explicit Lock/expiry still revokes work. During active stream verification, frontend idle timing follows the backend's actual activity instead of assuming unchanged uploaded bytes mean inactivity; absolute expiry remains fixed.

Windows generated incremental Rust cache reached 6.99 GB and a static-library archive failed with disk error112. Inspect free space before release builds. Reclaim only verified generated workspace paths when needed; preserve binaries/data/proof and normal tool caches. Do not respond by changing sandbox policy, deleting user files or downgrading tools.

Proof: [T017 acceptance](../evidence/F09-transfers-acceptance.md). Mobile/Android picker work remains S09; no phone proof is inferred.
