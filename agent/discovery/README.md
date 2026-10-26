# Discovery records

Use one file per meaningful finding: `D005-short-description.md`, continuing the ID sequence. Record facts that change implementation, constraints, tests or sequencing; do not create entries for routine successful commands.

Each entry needs: ID/title, date, state (`verified`, `hypothesis`, `resolved`, `superseded`), affected ticket IDs, **Finding**, **Evidence**, **Consequence**, **Follow-up**. Evidence names the dated command/file/official source and its small relevant result. Distinguish presence from functional proof. Update disproved hypotheses and link their resolution rather than erasing history.

Link important discoveries from affected feature packets and the checkpoint. Never paste keys, passwords, pairing/device secrets, raw protected configurations, secret environment values or full logs. Use safe summaries/fixtures.

## Seed findings

- [D001 Runtime selection](D001-runtime-selection.md)
- [D002 Rust host](D002-rust-host.md)
- [D003 Android prerequisites](D003-android-prerequisites.md)
- [D004 Migration baseline](D004-migration-baseline.md)

## Findings during tracker establishment

- [D005 Edit-approval observations and limits](D005-edit-approval-observations.md)
- [D006 Separate SSH catalog version error](D006-separate-ssh-catalog-error.md)

## Findings during implementation

- [D007 Native tool paths, sandbox context, UAC and Android prerequisites](D007-native-setup-boundaries.md)
- [D008 Android native-library symbolic-link permission](D008-android-symlink-permission.md)
- [D009 Tracker fixture preconditions after real ticket completion](D009-tracker-fixture-state.md)
- [D010 Android SDK platform package metadata](D010-android-sdk-package-metadata.md)
- [D011 SQLite ownership, recovery and concurrent writers](D011-sqlite-owner-and-writer-boundary.md)
- [D012 Migration schema-prefix and WAL-size review findings](D012-snapshot-review-guards.md)
- [D013 Desktop-first delivery and native WebView2 proof](D013-native-desktop-proof.md)
- [D014 Desktop operation foundation before phone integration](D014-desktop-guard-phasing.md)
- [D015 Strict operation decoding and cancellation admission](D015-operation-decoding-and-admission.md)
- [D016 Real OpenSSH trust/identity verification and native cleanup](D016-real-openssh-verification.md)
- [D017 Explicit desktop-first feature entry](D017-desktop-feature-entry.md)
- [D018 SFTP transport and transactional page lifecycle](D018-sftp-process-and-page-lifecycle.md)
- [D019 SFTP mutation outcomes and filename encoding](D019-sftp-mutation-outcomes.md)
- [D020 Desktop release complete; mobile is future development](D020-desktop-release-mobile-future.md)
- [D021 Transfer publication, cleanup and build-cache boundaries](D021-transfer-publication-and-build-cache.md)
- [D022 Windows agent proof and fresh sudo semantics](D022-windows-agent-and-fresh-sudo.md)
