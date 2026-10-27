# InfraWerf 0.1.2 — Complete desktop regression and legacy program removal

Date: 2026-10-27<br>
Revision: Working tree after `7f8b840`; final installed InfraWerf 0.1.2 artifact identified below.<br>
Platform: Windows x64 / Tauri 2 / WebView2, pinned mise tools.<br>
Result: pass

## Outcome

The reported inventory-refresh and Terminal failures are repaired. Read/session probes against the owner's working hosts 02, 03 and 04 each passed strict SSH verification, complete inventory update, authenticated SFTP setup and interactive Terminal connection. The prior Atlas installation and its obsolete launch shortcuts are removed. The current native store, Python reference and recovery snapshot remain byte-identical to the latest pre-test baseline.

## Repairs and independent review

[D046](../discovery/D046-desktop-retest-and-ssh-compatibility.md) records the verified causes: escaped Windows trust-file diagnostic paths, optional Linux hardware utilities, small source-clock leads, and Podman exposed through a Docker compatibility executable. Strict trust, fingerprint verification, authentication, timestamp validation and protected-session boundaries remain intact.

Additional fixes make local changed-key/connection errors classifiable, render structured SSH errors in the UI, update obsolete fixture call signatures, tolerate Windows scheduling in a timeout test, and scope/wait native UI assertions correctly. Production operation deadlines were not widened. The test VM was initially stopped; it was started for isolated fixtures and stopped again after all containers were cleaned up.

Independent bounded review passed the trust-path handling, clock wait and final checkpoint, collector fallbacks, compatibility detection, error mapping and fixture/test repairs. Temporary live protocol instrumentation was removed before the final build.

## Verification

| Layer | Actual result |
| --- | --- |
| Static and tracker checks | Rust/TypeScript contracts match, frontend types pass, Rust formatting/workspace checks pass. Tracker self-checks pass with 33 tickets, 14 rejection and two positive/phased-entry fixtures. Patch whitespace check passes. |
| Shared Rust workspace | 72 passed, zero failures. The two separately ignored real-fixture entry points were exercised explicitly below. |
| UI contract/model checks | Nine passed, including structured trust/authentication/session errors. |
| Strict system OpenSSH fixture | 14 cases passed: trust, changed/unknown key, public key/agent, authentication failure, certificate/fallback/alias, unreachable, proxy rejection, cancellation/limits and cleanup. Unknown/changed hosts stopped before authentication; configuration/trust hashes unchanged. |
| Actual SSH/SFTP/sudo fixtures | Both fixture entry points passed: 0/713-byte/8 MiB upload/download SHA-256 round trips, bounded streaming, mutation previews/confirmations, unusual names, cancellation/expiry/retry/lost acknowledgements, private protected staging and fresh sudo. The final elevation run additionally requires complete collection with both optional hardware utilities missing and a Docker-to-Podman compatibility wrapper; actual Podman rows must remain present. |
| Migration and recovery | Synthetic Python/Rust parity, rollback, writer exclusion and native import journey passed. |
| Native installed Windows journeys | All ten suites passed across sequential runs: desktop map/lifecycle, SSH, health, inventory, Files, file changes, Terminal, elevation, workload inspection/logs, and scoped workload control. The final collector received an additional native inventory rerun. Terminal resize is verified by polling actual remote dimensions after the asynchronous resize, rather than racing the debounce. |
| Live owner-host read/session checks | Hosts 02–04 all returned inventory `updated` and connected Terminal successfully using the existing identities/trust. Test databases were synthetic. No production file write, service control or sudo operation occurred. Server 1 was not contacted. |
| Packaging and startup | Release build/type checking passed, NSIS update exited 0, version 0.1.2 installed, ordinary `mise run start:desktop` opened the registered release and displayed the owner's four servers/four inventories/540 saved workloads. |
| Old program removal | Old executable absent in both standard and package-backed locations, Atlas uninstall registration absent, verified obsolete Start menu/Desktop links removed, InfraWerf entry retained. The repository launcher is now `InfraWerf Desktop.cmd`; it opens the registered release. |
| Owner data | Latest map/native database backed up before removal. SHA-256 of native store, Python reference and recovery snapshot unchanged after all tests/installations and the ordinary app restart; all three read-only SQLite integrity checks returned `ok`. Current app was reopened without test overrides. |

Native OS file-picker-specific evidence is reused from T017 because that plugin/selection path did not change; current transfer integrity/lifecycle and native Files/mutation flows were rerun. Tests of privileged changes and service/container control used disposable loopback fixtures, not owner workloads. This is comprehensive desktop regression evidence; Android/mobile remains future S09.

## Artifact and proof references

- Installer: `target/release/bundle/nsis/InfraWerf_0.1.2_x64-setup.exe`.
- Installer SHA-256: `EF1EAA0DFB70234B612F49BBD63364D090BC4C8251934D82FBB876270DCB9B62`.
- Installed executable SHA-256: `563EDEFD0E5A3235892AA6337C4CF9DB64D52464174D16EF27D1D90EC16EF715`.
- The existing unsigned-package and WebView2-bootstrapper distribution limits remain.
- Private sanitized summary: `.tools/full-desktop-proof/summary.json`. Per-suite logs, fixture result directories, live probe records, before/after data hashes, backup copies and final screenshot are under `.tools/full-desktop-proof/` and the linked ignored fixture directories.
- The new native runner is `mise run test:desktop:full` with `ATLAS_DESKTOP_BINARY` explicitly set. `scripts/verify_native_suite.ps1 -StartAt <suite>` resumes without repeating passed journeys. Owner app instances must be closed normally before fixture instances run.

## Closure

No requested work remains blocked. The installed primary app is InfraWerf 0.1.2. The old Atlas program is gone, while the original/reference and recovery data remain retained. The final owner map is open on the PC. Core ticket counts remain 25 done and eight deferred; these are post-release maintenance fixes, not activation of mobile, agents or rsync.
