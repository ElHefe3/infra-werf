# InfraWerf branding acceptance

Date: 2026-10-27<br>
Revision: Current working tree after `ae6f186`; release version 0.1.1. Core acceptance retains its original per-ticket revisions.<br>
Platform: Windows x64, Tauri 2 / WebView2, current-user installation.<br>
Result: pass

## Delivered

- InfraWerf name and “Your homelab, connected.” tagline, with an original forest-green/ivory W vector and accompanying wordmark.
- Shared SVG in the sidebar and loading state; favicon and Windows executable/installer icons generated from the same source. Header, footer, workspace dialogs and user-facing messages carry the new name.
- Product name/window title and aligned package, Cargo and Tauri versions at 0.1.1. Internal application identifier, executable name, schema, database location and transport contracts are preserved.
- Native proof harness selectors updated to the renamed Quit action and inventory receipt label; fixture contents and semantics remain unchanged.

## Proof

- `mise run brand:icons` generated the desktop icon assets from the SVG; raster icon inspected at 256 px. Future mobile asset directories were not copied from the temporary generator output.
- `mise run build:desktop` passed using the existing pinned Node 26.12.1, pnpm 12.15.0, Rust 1.99.1 MSVC and project lockfiles. The build includes TypeScript checking and Vite production bundling. Existing ts-rs attribute and chunk-size warnings remain nonfatal.
- `mise run exec -- node --test ui/client.test.ts ui/map-model.test.ts`: eight checks passed.
- `mise run tracker:check` passed: 33 tickets, 25 done and eight deferred, plus 14 rejection and two positive/phased-entry fixtures. `git diff --check` passed.
- InfraWerf setup exited 0. Installed executable version is 0.1.1. The original Atlas program remains installed: removing it was rejected by automatic approval review; installation proceeded in a separate program directory without uninstalling or overwriting it.
- Computer Use inspected the actual installed InfraWerf window. Its title, title-bar icon, sidebar mark, header wordmark and footer tagline rendered correctly. The existing four servers, four inventories and 540 workloads were displayed. Normal close/reopen through the new InfraWerf Start menu shortcut restored the same map and branding.
- SHA-256 checks after installation and after the first app review found the Python reference, native store and native recovery snapshot byte-identical to their pre-install values. No migration, map edit or remote operation was performed. Server 1 was not contacted.

Private proof files: `.tools/branding-proof/installation.json`, `data-before.json`, `data-after-review.json` and `infrawerf-desktop.png`. The Start menu shortcut target was inspected and points to the newly installed InfraWerf executable.

## Artifact identity and limits

- Installer: `target/release/bundle/nsis/InfraWerf_0.1.1_x64-setup.exe`.
- Installer SHA-256: `5FAF9FB270424AB1142134ED94CB9B0A5DEA1E3137211E7C61D7BFAA5A5450A4`.
- Installed executable SHA-256: `AC2B64F4C2E744BD99C0F8C9DF4423FCD53B0092E02851B1D294FEA5A62107A1`.
- Installed directory: `C:\Users\owner\AppData\Local\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\InfraWerf`.
- The package remains unsigned, and the existing WebView2 distribution limits remain. Mobile is future S09. No broader infrastructure or protected-operation regression was repeated for this presentation-only update.

The prior Atlas and new InfraWerf programs share the same app identity/data store; the old program is not an independent data backup. Use the InfraWerf Start menu shortcut. The three retained database files are the data preservation boundary.
