# D049 — draw.io embedding is a document integration

- Date: 2026-10-29
- State: verified (upstream documentation and local source inspection); native integration unproven
- Affected tickets: none activated; prospective diagram feature adjacent to F03 and F16

## Finding

The owner's `drawii.com` reference could not be resolved by the research tool. The provisional draw.io interpretation requires clarification. For draw.io, the official embed protocol provides a document editor via iframe and JSON messages; it does not automatically provide InfraWerf's server identity, monitoring or operation semantics.

The current React Flow map uses stable server IDs and workspace callbacks. Live status is derived independently from saved inventory. Consequently, replacing that map with an editor is larger than displaying or saving a diagram.

## Evidence

- 2026-10-29: `https://drawii.com` and its www variant were inaccessible through the web tool. This does not establish that the site is offline or does not exist.
- [Official embed protocol](https://www.drawio.com/docs/reference/embed-mode/): frame embedding, `postMessage`, load/save/autosave, exports and link events.
- [Official shape metadata](https://www.drawio.com/docs/manual/shapes/shape-metadata/): custom shape/connector properties can carry a binding.
- [Upstream README](https://github.com/jgraph/drawio): Apache-2.0 source, separate stencil/icon terms, and a test-distribution/production-offering distinction in the current README.
- Local inspection: `ui/main.tsx`, `ui/live-model.ts`, `ui/workspace-client.ts`, Rust `contracts.rs`, Tauri configuration and default capability file. No external editor frame is currently configured.

## Consequence

Prefer a bounded native feasibility proof, then document saving/export and linked server workflows. Keep SQLite and existing live state authoritative. Diagram deletion/renaming must not silently delete/reconfigure infrastructure records. Do not promise supported offline bundling or live canvas updates until proven.

## Follow-up

See [DRAWIO-ASSESSMENT.md](../DRAWIO-ASSESSMENT.md) for the proposed interface, architecture, delivery batches and focused checks. Confirm the intended product before activating implementation tickets. No app change or native test was performed for this finding.
