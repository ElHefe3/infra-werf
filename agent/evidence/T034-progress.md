# T034 progress — static checks and native monitoring journey

Date: 2026-11-07. Feature: F16 / S10. Ticket: T034 (in progress; this is not an acceptance record).
Revision: `dc66c0f` (WIP commit), clean working tree. Pins as in [CHECKPOINT.md](CHECKPOINT.md); Windows 11 x64, release build `target/release/atlas-native.exe` via `pnpm tauri build --no-bundle -- --locked`.

## Checks run

| Check | Result |
| --- | --- |
| `mise run check` (contract export check, `tsc --noEmit`, `cargo fmt --check`, `cargo check --workspace --locked`) | pass |
| `mise run test:feature -- T034` (`cargo test --workspace --locked --test-threads=1`, contract check, tsc) | pass: 79 passed, 2 ignored (real-SSH `elevation`/`files` fixture tests) |
| `node --test ui/client.test.ts ui/map-model.test.ts ui/live-model.test.ts` | pass: 11/11 |
| `mise run tracker:check` | valid: 34 tickets, done 25, in progress 1, deferred 8 |
| `node scripts/verify_native_monitoring.mjs` against the release binary | pass, 8 stages (below) |

## Native journey (actual WebView2/Rust, disposable store, loopback asyncssh fixture)

1. Configured aliases become candidates without any network connection.
2. Automatic SSH checks and approved baseline collection run on the topology; owner-offline is shown distinctly.
3. Periodic outage detection marks the card unreachable and keeps saved inventory.
4. A returning host recovers under its original stable server ID.
5. Workload changes are recorded by name/state; an identical refresh adds no inventory-change event.
6. A trusted replacement host key pauses collection until explicit reviewed-identity acceptance.
7. Archive drains checks, hides the map card, persists through restart and restores via the native form.
8. Ignore stops checks; deletion removes local history; rediscovery yields a fresh uncontacted candidate; the offline alias never authenticated.

Artifacts (ignored, untracked): `.tools/monitoring-proof-a4b6e7ac-a479-4fed-a78b-b44a027946fd/` (`result.json`, `servers.png`). No page errors.

## Environment notes

The single-instance plugin is unconditional, so the owner closed their running InfraWerf before the journey and it was reopened afterwards. No owner data, SSH trust, credentials or real hosts were touched; server 1 was not contacted.

## Not yet proven (T034 remains in progress)

- Schema 2 → 3 upgrade with pre-upgrade backup, plus restart and failure/recovery proof on controlled data (criterion 5).
- Permanent local deletion *preview* (criterion 4): the journey deletes via `remove_server` but does not assert a preview.
- History retention control in the native UI (criterion 3) beyond the Rust bound test.
- Independent review; build/install of the updated release; owner acceptance.
