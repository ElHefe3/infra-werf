# D069 — Triage of the first security-scan results

Date: 2026-11-20. Related: [D068](D068-glib-advisory-is-linux-only.md), `.github/workflows/security.yml`.

## Context

The scheduled scan workflow (CodeQL, OSV-Scanner, Semgrep, secrets) first ran on 2026-11-19 and produced about 150 code-scanning alerts. Tracking issues "Security review batch 01" to "24" were opened for them. 55 alerts were open on 2026-11-20.

## Result

No alert was a vulnerability in the Windows release.

| Alerts | Tool and rule | Verdict | Action |
| --- | --- | --- | --- |
| 19 | Semgrep `html-in-template-string`, in `static/` (the retired Python reference UI) | False positive. Every user-controlled value goes through `esc()`. The other interpolations are numbers, fixed strings or `esc()`-built fragments. | Dismissed. `scripts/test_legacy_render_security.cjs` already checks hostile identifiers render as inert text and passes. |
| 23 | Semgrep `unsafe-usage`, in `terminal.rs`, `tunnel.rs` and the example job | Required Windows FFI (ConPTY, job objects) and the wasm guest interface. Handles are owned by RAII wrappers, terminal sizes are bounded (2 to 500 columns, 2 to 300 rows) before the `i16` casts, and handle ownership moves exactly once. | Dismissed as won't fix. |
| 1 | Semgrep `args`, in `export-contracts.rs` | The dev tool only looks for `--write`. | Dismissed. |
| 6 | OSV-Scanner: `glib`, `proc-macro-error`, `zvariant`, `zbus_names`, `zvariant_derive` | Linux-only; not built for Windows. See D068. | Dismissed. |
| 1 | OSV-Scanner: `chrono` is unmaintained (RUSTSEC-2026-0543) | Not a vulnerability. `chrono` is used across `atlas-core`. | Dismissed. Moving to `time` or `jiff` is a possible later migration. |
| 3 | CodeQL `py/path-injection`, in `scripts/plugin_https_fixture.py` | The test fixture checked the path with a regex and a parent comparison that CodeQL does not recognise. | Fixed: the fixture now looks the name up in a map of existing files and never builds a path from the request. |
| 2 | Semgrep path rules, in `scripts/test_legacy_render_security.cjs` | A loopback test server joined the request path onto a folder. | Fixed: files are read once at startup and served from a map. |

## Verification

The fixture was started and queried: existing archives return 200, and `/../`, encoded `..` and missing names return 404. The legacy render test passes. The five fixed alerts stay open until GitHub scans the new commit.
