# D036 — Migration parity must account for authenticated inventory provenance

Date: 2026-10-26<br>
Affected: T024

## Finding

The older Python/Rust migration parity harness compared its approved baseline projection against the entire Rust inventory object. T020 later added nullable `source_binding` and `source_host` fields for authenticated refresh provenance. Rust correctly returned those fields as `null` after migrating a legacy report, but the stale comparator treated the two additional fields as a data mismatch.

## Resolution

Compare approved baseline fields on both sides, then separately assert that legacy migration does not invent authenticated source provenance. The failed run used only its generated synthetic fixture and is retained under the ignored `.tools/data-proof-*` directory. No user database or server was accessed.
