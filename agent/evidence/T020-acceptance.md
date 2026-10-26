# T020 — Approved SSH inventory refresh

Date: 2026-10-26<br>
Scope/feature: S05 / F07<br>
Revision: `7c2e941` plus the uncommitted T018–T020 desktop changes; no commit was created.<br>
Result: pass

## Outcome

The Windows desktop can deliberately refresh the approved static baseline through authenticated SSH. Atlas issues a fixed read-only Linux command, filters remote output to approved fields, and writes a complete newer snapshot by stable server ID. It retains the source collection time separately from Atlas's receipt time. Partial output, stale/equal source times, and SSH authentication failure leave the last approved snapshot and its event history unchanged.

## Acceptance

- [x] A complete authenticated refresh preserves server/workload identity, owner-authored names/host/layout, relationships, creation times and source collection time.
- [x] Partial or older/equal output does not replace the saved observation. The UI distinguishes partial, unchanged and failed refreshes.
- [x] Remote descriptions/raw output are excluded from the saved baseline. The controlled test confirmed a synthetic service description was absent from SQLite.
- [x] Refresh is user-triggered and fixed to approved system details, service summaries and container summaries. It does not use sudo, collect process/log/configuration data, or add a periodic collector.
- [x] The native failure journey shows SSH authentication failure and confirms inventory, source timestamp, event history, SSH config and trust remain unchanged.

## Verification

- `cargo test -p atlas-core --lib inventory:: --locked`: **2 passed**, including malformed numeric values followed by valid duplicates, contradictory empty-container markers, partial reports, unknown rows and future timestamps.
- `cargo test -p atlas-core --test inventory --locked`: **5 passed**, including stable identity, owner edits, relationships, source-time ordering and excluded-field persistence checks. On this Windows sandbox, the temporary SQLite test target required process-level escalation; see [D029](../discovery/D029-windows-temp-sqlite-sandbox.md).
- `cargo fmt --all`; `pnpm check`; `node --check scripts/verify_native_inventory.mjs`; and `python -m py_compile scripts/linux_fixture.py`: passed. Tauri build also ran TypeScript checking and Vite production build.
- `pnpm tauri build --debug --no-bundle -- --locked`: passed on the pinned Windows/MSVC toolchain; built `target/debug/atlas-native.exe`.
- Native Windows Tauri/WebView2 journey: **passed** against disposable loopback OpenSSH only. Proof: `.tools/native-inventory-proof-62774b3f-ac81-47aa-8197-87c3c0057d82/result.json`. It verified a successful approved refresh, stable map fields and excluded data, partial-result retention, and authentication-failure retention. No real host, sudo, owner database, SSH trust change or app page error was involved.
- Bounded independent review: **Pass**. The review found and the implementation closed a target/write race and strict-parser gaps before the final native proof; the reviewer checked the post-repair source and evidence.

## Limits and follow-up

The actual remote journey used the project's Debian/OpenSSH fixture with synthetic `systemctl` and Podman output. Hosts without the required Linux tools or complete approved output report a partial result and keep the last snapshot. This does not prove compatibility with every owner host or complete S05/release acceptance. T021–T026 remain open; Android stays in future S09.
