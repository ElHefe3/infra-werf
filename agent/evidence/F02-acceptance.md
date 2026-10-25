# F02 / S01 — Contracts, persistence and classified import acceptance

Date: 2026-10-25
Revision: 23663d2 plus the tested F02 working tree (atlas-core, generated bindings/adapters, data CLI/harness and final migration guards).
Result: pass

## Acceptance mapping

- **T003:** shared Rust contract version 1 generates TypeScript used by both client adapters. Stable IDs, distinct error codes, unknown metrics, freshness, separate reachability/health, session/operation handles and baseline/protected result types passed focused serialization, input and version-handshake checks. No credential fields or authentication implementation were added. See [T003 proof](F02-T003.md) and [CONTRACTS.md](../CONTRACTS.md).
- **T004:** native SQLite v2 supports validated CRUD and restart parity. Migration opens its source read-only, retains full legacy recovery, rebuilds canonical tables in staging and publishes a separate destination without clobbering owner files. IDs, owner edits, layout, relationships, timestamps and supported history survive. Failed/unsupported migrations remain recoverable; a retained SQLite lock prevents concurrent Python/Rust writers. See [initial T004 proof](F02-T004.md) and [STORAGE.md](../STORAGE.md).
- **T005:** [INVENTORY.md](../INVENTORY.md) classifies retained/excluded/authenticated fields. Bounded atomic imports preserve source identity/owner edits, ignore equal/older timestamp instants, and reject invalid/conflicting batches without changes. Direct SQLite tests prove raw/config/warning/credential fields never enter stored overview JSON. Snapshot view models carry saved/stale/unknown labels without invented live health; native map rendering remains S02.
- **S01:** [F01 acceptance](F01-acceptance.md) proves the Windows/Android native-build half. F02 completes contract/persistence/import parity, classification, recovery and risk review. This does not claim phone execution, native map rendering or live cutover.

## Final verification

- `mise run test:feature -- F02`: **16 Rust tests**, **2 adapter tests**, binding/type checks and tracker fixtures passed.
- `mise run test:integration`: actual Python reference and Rust CLI passed synthetic parity, rollback, Python-writer exclusion and newer/equal/older/invalid import journeys.
- After the review fixes, `python scripts/verify_data.py --legacy-source data/atlas.sqlite3` through mise repeated the affected journey on private copies. Synthetic counts: 2 servers / 1 edge / 5 events. A read-only backup of the real reference: 4 servers / 0 edges / 4 events. Parity, restored legacy state and separate Python-writer rejection passed; **original_unchanged=true** by SHA256. Final private copies: ignored `.tools/data-proof-f7957dc76b59438182024c7a9bf7086f/`.
- `mise run check`: non-rewriting binding comparison, TypeScript, Rust formatting and locked Windows workspace checks passed.
- `mise run check:android-core`: portable core including bundled SQLite compiled for Android ARM64 with the existing NDK/API 24 compiler. No APK rebuild/UAC was needed for these core checks.

## Review and refinement

One independent reviewer inspected changed code and safe evidence only, without accessing live data/private copies. It found two P2 guards: SQL LIKE's underscore could exempt `sqliteExtra`, and a main-file size check missed WAL snapshot growth. Literal prefix matching, backup-page/completed-file bounds and small regressions fixed both. Follow-up confirmed **both resolved; no remaining material issue in the fixes**. [D012](../discovery/D012-snapshot-review-guards.md) records the findings.

Compilation caught an unsigned SQLite getter during the size fix; the final code reads the signed SQLite value and validates conversion. Known ts-rs Serde-attribute warnings remain visible; reviewed generated representations and focused input/serialization tests cover them.

## Limits and preservation

Original Python source/UI/database and configured paths remain unchanged. All migration/import/rollback writes used new controlled copies; outputs and private proof material stay ignored. No proof child is left running. No SSH, infrastructure mutation, network listener, release signing, phone installation or cutover occurred.

The native window/APK remains the foundation composition. The coordinator adapter requires an injected authenticated transport; F05 supplies the real connection. S02 implements the React Flow desktop/mobile map. Release/device/owner acceptance remains T024–T026. Existing F01 artifact proof is reused; the earlier APK is not claimed to contain F02's new map or storage UI.
