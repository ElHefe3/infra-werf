# Desktop regression and legacy program removal

Date: 2026-10-27<br>
Revision: Starting at `7f8b840` / installed InfraWerf 0.1.1.<br>
State: complete — see [desktop retest acceptance](DESKTOP-RETEST-acceptance.md).

## Authorized outcome

The owner requested a complete desktop test after inventory refresh and Terminal connection failures, plus removal of the prior Atlas program. Preserve the latest owner map, native store, Python reference and recovery snapshot. Server 1 remains powered off/corrupt and must not be contacted. Actual server 2 SSH read/session probes are diagnostic; file writes, service control and failure injection use isolated fixtures.

## Acceptance and proof plan

- [x] Reproduce the reported inventory and Terminal failures and identify their cause; record repairs and regression cases.
- [x] Run static/contract/type checks, Rust workspace and real SSH/SFTP/sudo/transfer fixtures, UI contract/model checks and data migration/recovery checks.
- [x] Exercise installed Windows desktop journeys for map, SSH, inventory, terminal, health, files/mutations/transfers, elevation and workload inspection/control.
- [x] Remove only the old Atlas installation/registration/shortcuts, preserving the new InfraWerf program and all databases; verify hashes after removal.
- [x] Install the corrected release if needed, verify the latest owner map and working live read/session flows, and record remaining limitations honestly.

Independent bounded review covers the shared SSH setup and any repairs. Test outputs and the latest native/recovery backups are retained under ignored `.tools/full-desktop-proof/`. The app is closed normally before test instances run; native suites run sequentially to avoid process/port conflicts.

## Initial findings

The full static check found pre-existing Rust formatting drift and stopped at rustfmt. Contract generation and TypeScript checking passed before that point. This formatting issue is separate from the owner-reported runtime failures.
