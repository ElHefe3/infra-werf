# D001 — Project runtime selection is not reproducible yet

Date: 2026-10-25
State: verified
Affected: T001, T002

## Finding

Inherited `node --version` resolves to `v14.18.1`. The separately inspected bundled Node is `v24.22.1`. Mise `2026.9.1` is installed; the global tools section requests `node = "latest"`. No project-local mise/tool-version file was found in Atlas during inspection.

## Evidence

Read-only version commands, a scoped tool-file search and filtered global configuration inspection on 25 October 2026. Mise also emitted a remote version-resolution DNS warning during version checks. No runtime was installed or configuration changed.

## Consequence

Tool presence does not establish the effective runtime. Bare PATH commands, hardcoded bundled paths and a floating global request do not provide the owner's latest-stable reproducible project setup.

## Follow-up

T001 resolves latest stable Node Current, pnpm, Rust and compatible dependencies, verifies effective versions inside mise and commits exact project pins. Reuse resolved versions/caches; record compatibility constraints rather than silently falling back. Sources: [mise pinning](https://mise.jdx.dev/cli/use.html), [configuration](https://mise.jdx.dev/configuration).
