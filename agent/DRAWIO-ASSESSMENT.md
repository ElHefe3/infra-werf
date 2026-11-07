# draw.io integration assessment

Date: 2026-10-29. Status: research and proposed design; no application changes or new delivery tickets.

The owner asked what implementing `drawii.com` would look like. That exact site could not be opened by the research tool, and its identity is unverified. This assessment provisionally interprets the request as **draw.io / diagrams.net**, pending the owner's clarification. It must not be treated as evidence about another product called drawii.

## Recommendation

Start with a **Diagram design** view alongside InfraWerf's live topology. Use draw.io for richer layout and documentation, and keep InfraWerf's existing server identities, monitoring, history and authenticated operations authoritative. Prove the embedded editor in the actual Windows application before committing to an editor replacement.

This provides shapes, annotations and architecture drawings without making live monitoring depend on an external editor. Replacing React Flow entirely is a larger, separately scoped change because our current nodes are interactive application controls with live status and lifecycle behavior.

## What the owner would see

Proposed desktop composition:

```text
InfraWerf       Live map | Diagram design | Inventory | Servers | Activity

Diagram design: Homelab             [Generate from current map] [Save] [Export]
+-----------------+--------------------------------+------------------------+
| Shape libraries | Editable architecture drawing | Linked InfraWerf item |
| Network         |                                | lab-server-2             |
| Servers         | Router -- Switch -- Servers    | Current status         |
| Storage         | Sites, groups, notes, links    | Last check / inventory |
| Basic shapes    |                                | [Open workspace]       |
+-----------------+--------------------------------+------------------------+
```

The right-hand panel is InfraWerf UI. Selection integration needs a native prototype; a documented, intercepted shape link is a simpler initial route to opening a linked server than assuming every editor click is exposed by the embed API.

Expected behavior:

- Generate a starting diagram from saved active servers, their layout and existing relationships. Show the snapshot time. A drawn connector remains a documented relationship; it does not establish a measured network link.
- Add routers, storage, network zones, notes and groups as drawing objects. Unlinked shapes carry no monitoring claim.
- Associate a shape with an existing stable server ID. Renaming a label does not change its SSH alias or create a second server. Multiple shapes may represent one server.
- Open Files, Terminal or Services through the existing InfraWerf workspace and authentication flow.
- Show current reachability and freshness in the linked-item panel. Lab-server-1 remains owner-reported offline and is never contacted to populate a drawing.
- Keep a missing/archived/deleted server binding visibly unresolved or archived. Deleting a shape removes its diagram representation; server deletion remains a separate explicit operation.
- Save the editable document locally and export `.drawio`, SVG or PNG. A static export is a dated snapshot, with historical and current observations labelled appropriately.

## Verified upstream capabilities

The official editor supports iframe embedding at `https://embed.diagrams.net`, JSON messages through `postMessage`, initialization/load/save/autosave, and XML/image exports. It also documents merge/patch operations and intercepted link events. These are integration primitives, not proof that our complete live workflow works in WebView2. [Embed protocol](https://www.drawio.com/docs/reference/embed-mode/).

Shapes and connectors can carry custom metadata. An `infrawerfServerId` attribute can preserve a proposed binding through XML saves; this is an application design choice. Metadata is user-editable, so bindings must be checked against the local workspace. [Shape metadata](https://www.drawio.com/docs/manual/shapes/shape-metadata/).

Upstream source is Apache-2.0, with separate stencil/icon terms. The current repository README describes that distribution as a test version and points to a paid production offering. Check the intended distribution route and the exact release's notices before choosing offline bundling; do not assume a free, supported production package from the license label alone. [Upstream repository](https://github.com/jgraph/drawio).

## Fit with the current implementation

| Area | Existing implementation | Proposed work |
| --- | --- | --- |
| Topology | `ui/main.tsx`: React Flow server cards, drag persistence, connection editing and workspace actions | Keep the live map initially; add a separate editor component |
| Identity and relationships | `MapServer.id`, `Relationship` in Rust contracts | Map document cell IDs to existing IDs; never match by display name alone |
| Live state | `ui/live-model.ts`: owner-offline, reachability, freshness and lifecycle filtering | Reuse these rules in the linked-item panel; never accept health values from imported XML |
| Storage | Rust/SQLite owns the workspace | Add versioned document storage, bounded recovery revisions and explicit migration proof |
| Native transport | `ui/workspace-client.ts` invokes typed Rust commands | Add narrowly scoped document commands; no generic command forwarding from editor messages |
| Embedding | Current CSP permits local resources and IPC; no external frame origin is configured | Permit only the chosen editor frame origin for the prototype and verify Windows behavior |

Suggested new components: `ui/diagram-editor.tsx` for lifecycle/messages, `ui/diagram-model.ts` for document/binding validation, Rust document persistence plus generated contracts. These are proposed boundaries, not files created by this assessment.

Keep author-owned layout separate from volatile observations. A polling refresh must not reload the whole editor, move a shape under the cursor or turn health updates into persistent user edits. Initial live status belongs in the InfraWerf panel; live recoloring inside the editable canvas is optional later work with its own synchronization proof.

## Hosted versus local editor

| Route | Benefit | Constraint |
| --- | --- | --- |
| Hosted embed | Smallest useful integration prototype; documented endpoint | Editor needs internet and receives the diagram inside a remotely served frame; editor version may change independently |
| Bundled/local editor | Potential offline editing and pinned assets | Distribution/production route, package size, assets and update handling need investigation and native proof |
| `.drawio` export first | Smallest standalone deliverable; current live map remains available | External edits do not round-trip into InfraWerf unless later import/binding work is implemented |

A hosted editor does not inherently require cloud document storage. Host-managed local saving is part of the proposed integration. Nevertheless, loading remotely supplied editor code is a different dependency from today's local-only interface.

The parent checks both message origin and frame identity, validates message types and size limits, and exposes only document actions. It never sends credentials, session handles, protected file contents or terminal output to the editor. Keep remote Tauri command access disabled; Tauri documents that remote API access is a separate capability configuration. [Tauri capabilities](https://v2.tauri.app/security/capabilities/).

## Efficient implementation sequence

These are proposed functional batches, not activated tracker tickets:

1. **Native feasibility proof:** load a synthetic diagram in the installed Windows WebView, edit, receive save, intercept an internal link, export, close/reopen the editor, and handle unavailable internet. Check keyboard focus, clipboard and download behavior. Stop and reassess if isolation or reliable saving cannot be achieved.
2. **Local documents and export:** generate from a map snapshot, save/reopen XML, preserve bindings and geometry, provide bounded recovery, export files through native selection. Test migration/rollback and failed writes using temporary data.
3. **Linked server workflow:** open an existing workspace from a linked shape; refresh the external status panel without resetting the drawing. Cover rename, duplicate representation, missing/deleted server and owner-offline cases.
4. **Optional deeper integration:** editable import reconciliation and live canvas styling. Preview changes before altering operational records; prove monitoring updates cannot overwrite in-progress edits. Consider replacing the main canvas only after this behavior is accepted.

Use small model/message tests for malformed input, wrong-origin messages, stale revisions and invalid bindings. Run one controlled native end-to-end journey per delivered batch. Broaden regression only when shared persistence/contracts or native operation routing change. Reuse existing SSH/monitoring proof instead of probing the real servers for drawing tests.

## Decision boundary

Embedding looks technically feasible from the official protocol and our React/Tauri architecture. Native compatibility, the chosen offline distribution route and live edit synchronization are **unproven**. This assessment changes no installed application, database, dependencies or active tracker status. Mobile remains future development.
