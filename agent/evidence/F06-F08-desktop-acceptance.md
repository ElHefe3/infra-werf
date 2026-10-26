# Protected desktop sessions and SFTP browsing — T014 / T015

Date: 2026-10-26

Revision: cb18c48 + reviewed desktop SFTP working tree; the local completion commit contains this record.

Result: pass

## Included journey and environment

T014 integrates the shared operation guard with actual desktop authentication, ephemeral sessions and cleanup. T015 supplies read-only directory navigation/stat. This is the explicit desktop phase in [D017](../discovery/D017-desktop-feature-entry.md); S02–S04 and phone/pairing/sudo/release gates remain open.

Windows x64, actual Tauri/WebView2/Rust IPC, system Windows OpenSSH 9.5p2, fixed SFTP v3 subsystem. Exact existing mise pins: Node 26.12.1, pnpm 12.15.0, Rust/Cargo 1.99.1 MSVC, Python 3.15.1; React 19.3.1 and Tauri Rust 2.12.5. Test identities, trust files, loopback listeners, SQLite stores and WebView profiles are private synthetic fixtures. The existing pinned AsyncSSH runtime is reused. No real server files/keys/trust/config are test targets.

## T014 acceptance mapping

- **Authentication/authority:** Genuine publickey success and the SFTP handshake precede the broker session grant. None authentication, rejected credentials, unknown/changed host trust, forged resources, wrong server/session and unrecognised fields/commands fail closed. Session/resources belong to the same backend principal. Existing broker regressions cover unauthenticated/foreign/revoked authority and late delivery; real native proof repeats host/handle failures. Paired transport proof stays T010/T011.
- **Lifetime/clearing:** Defaults are five-minute idle and 30-minute absolute limits. `mise run test:files` independently shortens each limit and proves expiry, lock/revoke denial and actual child/lease cleanup. The native journey proves explicit UI lock, backend server-edit invalidation, leaving Files and actual disconnection clear listings/metadata/authentication UI. Backend closure notifications cover every close path; the frontend also handles document hiding and its expiry timer. No automatic credential/session replay.
- **Bounded execution:** Purpose-specific commands accept typed IDs/actions; paths remain backend-owned bytes, with no shell interpolation. The fixture includes a shell-looking filename and non-UTF-8 bytes. Single-use bootstrap tickets/cursors, expired page resources, timeout, cancellation, wire/output limits and native close admission are exercised. A timeout returns while cleanup remains admitted until the process/pipe threads finish. Unknown operations, writes and sudo are unavailable.
- **Retention:** Broker audit contains only operation/server IDs and a known error code. Files/results/diagnostics have no SQLite/public-inventory route. Read-only journeys preserve database/map bytes outside the harness's two explicit synthetic host edits used to test invalidation. Both known-host files and SSH config hashes remain unchanged. Fixture counters record zero shell commands and zero file-content/write opens; native UI has zero page errors. There is no mutation to duplicate on cancellation/retry.

## T015 acceptance mapping

- The actual native journey covers explicit root navigation, authenticated home, an 18-level nested path, all 350 large-directory entries through bounded pages, space/Unicode/non-UTF-8 names, exact u64 size strings, permissions/owner/group metadata, symlink identity, explicit link-to-directory following, broken links and permission-denied directories.
- Breadcrumb selection, metadata selection, refresh and paging work through real Rust IPC and SFTP. Each page has at most 100 entries and stays within response budgets; resources/cursors are bounded. Display names are never round-tripped to paths. Old page resources/cursor replays fail.
- Protected data clears on lock, backend invalidation and disconnect; core proof covers idle/absolute expiry and revocation. Compact-phone selection/breadcrumb/clearing acceptance remains explicitly T009/T010, with physical-device acceptance T026. No phone journey is inferred from desktop proof.

## Proof and artifacts

- `mise run test:feature -- T014`: **38 Rust tests** (36 core + 2 native), **4 client/model tests**, shared bindings/type checks and tracker fixtures passed. The separate real-SFTP integration test is intentionally ignored by the ordinary suite and run explicitly below.
- `mise run test:files`: **1 actual-system-SFTP integration test passed**, including denied-navigation recovery, stale cursor/resource denial, lock/revoke, independent idle/absolute expiry, timeout, child cleanup and closure-event metadata.
- `mise run test:files-native`: pass. Final private artifact directory `.tools/native-files-proof-b0f9424c-c01e-4a4f-a476-81f154686705/`, containing result metadata and home/large-page screenshots. Native screenshot visually inspected. Two deliberate private-map host edits test backend invalidation and restore the fixture alias; other reads preserve the expected map/database state.
- `mise run test:ssh-native`: pass; `.tools/native-ssh-proof-19ca02f9-cc20-4bca-bc42-296dfef9f5ff/`. Trusted/unknown/changed/rejected/unreachable/proxy/certificate/none/overflow/cancel and admission regressions pass with unchanged fixture config/trust/database.
- `mise run test:desktop`: pass; `.tools/desktop-proof-dfba0029-8fc5-43c0-ac8a-1d7e3535ba77/`. Two laptop sizes, saved 540-workload inventory, search, keyboard/mouse editing, single instance, native close/restart, errors and persistence pass. Files entry requires explicit authentication; other unavailable controls remain honest.
- `mise run smoke:desktop`, `mise run check`, `mise run check:android-core`: pass. Only the previously documented ts-rs attribute warnings remain; new platform-unused warnings were removed. No Android APK or installer/release rebuild was needed for this desktop phase.
- Tracker self-test: **14 rejection fixtures + 2 positive completion/phased-entry fixtures**, with the ordinary full-scope gate preserved.

## Failures, review and repairs

The first harness launch raced the initial WebView navigation; it now waits for the app's ready UI. Actual native navigation exposed OPENDIR succeeding before a denied READDIR: resources/pages now commit only after the first new page succeeds, preserving the previous directory after denial. Focused actual-SFTP and native regressions pass.

Independent review reported two P2 findings: pipe-thread creation before child ownership, and explicit backend closures lacking view notifications. `PipeProcess` now owns the child/lease before fallible setup. A Windows regression injects failure at each pipe spawn, verifies the OS process handle is signalled, confirms the started thread joined and the lease released. Every close now queues bounded metadata; overflow clears all protected views. The native server-edit test proves clearing without the component's Lock handler. Reviewer confirmed both repairs resolved with no further blocker, followed by the passing final native run.

The map regression's old Files-unavailable assertion was updated to the new explicit authentication entry; all remaining map assertions passed. No product gate was removed to make a test pass.

## Limits, cleanup and rollback

Scope is the SSH account's filesystem; this does not claim a restricted-root jail. SFTP runtime is Windows-only; shared contracts/core compile for ARM64 Android. Existing loaded OS-agent compatibility, paired/coordinator policy, fresh sudo, mutations, content transfers, terminal, installable release and actual phone/owner acceptance remain their tickets. The owner reports other real hosts passing SSH verification; that observation is not a substitute for those remaining gates. Server 01 was confirmed offline due to corruption; no address/trust rewrite was attempted.

Fixture apps/listeners/children are shut down by the harness. Normal Atlas was reopened: one native process with window title Atlas, zero listeners on the three test CDP ports. The reference database's SHA256 remained `2CD22BD9AFBE8BF95481C5FA2D33A9A4773BEF6743D278875FB2CB68316260CE` before/after reopening. The tested native executable SHA256 is `A65516722B747F335AA17F452C8E2ADD8176CAE639737633C7E2AB49B21741C7`. Protected results are not migrated into the reference. Rollback is the prior native build plus preserved native/public data; no remote state was changed by this work. Keep ignored private proof directories out of Git.
