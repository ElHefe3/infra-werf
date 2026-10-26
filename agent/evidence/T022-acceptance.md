# T022 acceptance — Protected workload inspection and logs

Date: 2026-10-26<br>
Revision: `7c2e941` plus the uncommitted final T022 working tree.<br>
Result: pass<br>
Feature/ticket: F11 / T022

## Environment and target

- Windows x64 desktop, native Tauri/WebView2 application.
- Disposable Debian/OpenSSH fixture in the local Podman machine; its SSH endpoint is loopback-only. The owner’s servers, including powered-off server 1, were not contacted.
- Rust 1.99.1 MSVC, Node 26.12.1, pnpm 12.15.0, Tauri CLI 2.12.4.

## Acceptance mapping

| T022 criterion | Evidence | Result |
| --- | --- | --- |
| System/user services and supported containers resolve only in their authenticated host, account and runtime scope | Rust tests bind IDs to Atlas server, scope, runtime and name, and bind saved inventory to the resolved SSH alias/host/user/port and verified key. The native journey inspected system and user systemd, user Podman and system Docker; unknown IDs and another server were denied. It then changed the SSH account while retaining alias and key; no old workload IDs were issued and the UI asked for a fresh inventory. | Pass |
| Log selection/output is bounded and stopped, denied and unavailable states are explicit | The fixed adapters request at most 50 log lines and the UI receives at most 8 KiB of UTF-8 text. Native service/container paths covered running, stopped, permission denied and unavailable outcomes. A multibyte fixture split a character at the remote byte-tail boundary; the UI still showed a bounded tail and the trimming notice. | Pass |
| Protected details/logs never persist in overview, telemetry or acceptance artifacts | Native proof confirmed Lock removed the visible result, SQLite/WAL/SHM stayed byte-identical across protected reads, the server snapshot and event history were unchanged, and synthetic log markers appeared in neither the database nor proof JSON. | Pass |

## Verification

- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test -p atlas-core --lib --test inventory --locked --offline`: **29 unit tests and 6 SQLite integration tests passed**. The integration tests required elevated local execution because the default Windows sandbox blocks their temporary SQLite files; see [D029](../discovery/D029-windows-temp-sqlite-sandbox.md).
- `cargo run -p atlas-core --bin export-contracts --locked --offline -- --write` and `--check`: passed. Generated TypeScript contracts were refreshed and match Rust definitions.
- Direct pinned Node checks: TypeScript `tsc --noEmit` passed; client/map tests **7 passed**; Vite production build passed with the existing 500 kB chunk-size warning.
- Windows Tauri debug build with `--no-bundle`: passed.
- `python agent/check_tracker.py --self-test`: 33 tickets, 15 features and 9 scopes valid; 14 rejection and 2 phased-entry fixtures passed. `git diff --check`: passed with no whitespace errors.
- Final native Windows journey passed: [sanitized result](../../.tools/native-workload-proof-46e20abd-88a1-4649-ac49-fc980526754f/result.json). It covered the full protected inspection UI, profile mismatch, Unicode-safe cap, clearing and persistence checks. No page errors, sudo or real host.
- Independent bounded review: **Pass** after the reviewer’s alias/account/key provenance and split-UTF-8 findings were resolved. No remaining findings in the reviewed T022 slice.

## Repairs and limits

The first native pass found a synthetic systemd matcher that omitted the fixture’s `atlas-` unit prefix. A later pass exposed a truncation-flag boundary at an exactly full tail. Review then found alias-only provenance and a lossy UTF-8 expansion path; T022 now binds inventories to the resolved SSH profile and verified key, compares complete snapshots before equal-time rebinding, and enforces the displayed byte cap at a UTF-8 boundary. These regressions are covered by unit, SQLite and native UI journeys; see [D031](../discovery/D031-workload-inventory-host-binding.md), [D033](../discovery/D033-workload-identity-binds-ssh-profile.md) and [D034](../discovery/D034-utf8-safe-log-bounds.md).

The proof uses synthetic systemctl/Podman/Docker output and a loopback SSH fixture. It does not claim compatibility with every real host. Workload controls remain T023; mobile is future S09. `mise run` could not resolve pinned tools under this sandbox, so the exact installed binaries were invoked directly; see [D032](../discovery/D032-sandboxed-tool-resolution-and-loopback-fixtures.md).
