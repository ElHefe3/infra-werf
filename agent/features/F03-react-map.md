# F03 — The approved Atlas experience in shared typed React components, using React Flow for map mechanics.

<!-- atlas-feature: {"id":"F03","scope":"S02","depends_on":["F02"],"deferred":false} -->

## Work packet

**Outcome:** The approved Atlas experience in shared typed React components, using React Flow for map mechanics.

**User journey:** Load persisted map → search/edit/connect → inspect workspace → reload on the Windows laptop. Owner steering on 2026-10-25 prioritises desktop; compact-phone composition and touch checks move intact to T009, without removing them from S02 or the core release.

**Scope:** [S02](../scopes/S02-applications-map.md). **Feature prerequisites:** [F02](F02-domain-persistence.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D004](../discovery/D004-migration-baseline.md), [D013](../discovery/D013-native-desktop-proof.md).

## T006 — Replace handwritten map with React Flow

<!-- atlas-ticket: {"id":"T006","feature":"F03","status":"done","depends_on":["T003","T004","T005"],"evidence":"../evidence/F03-acceptance.md"} -->

### Outcome and included work

Port real server/relationship editing to React Flow with persistent domain state.

### Exclusions

Visual redesign, automatic infrastructure rewiring or porting obsolete demo behavior.

### Implementation and contracts

Preserve approved server-card styling, IDs and desktop layout. Support custom server nodes, directed labelled relationships, selection/pan/zoom and real persistence. Keep presentation separate from domain coordinates so T009's compact mobile arrangement will not overwrite them.

**Ticket dependencies:** [T003](F02-domain-persistence.md), [T004](F02-domain-persistence.md), [T005](F02-domain-persistence.md).

### Acceptance criteria

- [x] Add/edit hosts, move desktop nodes and create/remove relationships survive reload/restart.
- [x] Desktop search/focus/selection and pan/zoom work without unintended edits or nested-service identity loss. T009 retains touch pan/zoom acceptance.
- [x] All displayed state comes from the domain/approved inventory; no simulated successful operations or invented live values.

### Verification

Focused map-model tests and one desktop save/reload/restart journey with a controlled fixture. Touch interactions are verified in T009; physical device proof remains T026.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T006, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **passed** in [F03 acceptance](../evidence/F03-acceptance.md), including native reload/restart and reviewed keyboard regressions. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record a layout/contract mismatch before changing preserved identities or flattening owner data.

## T007 — Port shared layouts and server workspace

<!-- atlas-ticket: {"id":"T007","feature":"F03","status":"done","depends_on":["T006"],"evidence":"../evidence/F03-acceptance.md"} -->

### Outcome and included work

Reproduce the approved desktop interface using reusable React components. The separate compact phone composition is retained in T009.

### Exclusions

Floating-window OS emulation, new themes or claiming browser viewport checks prove native Android.

### Implementation and contracts

Provide inspector plus Files/Services/Terminal/System/Settings/Transfers workspace with tabs, reusable status/error components and responsive laptop navigation. Use adapters so platform-specific transport/file handling stays outside presentation. T009 owns the compact phone navigation and arrangement.

**Ticket dependencies:** [T006](F03-react-map.md).

### Acceptance criteria

- [x] Laptop layouts preserve the approved look with readable content and no document overflow. Narrow-phone checks remain in T009.
- [x] Search and full inventory browsing work with the existing large report shape; loading/offline/empty/error states are truthful.
- [x] Desktop workspace controls are accessible by keyboard and unimplemented operations are labelled unavailable until their tickets pass. T009 retains touch accessibility.

### Verification

One component/journey set at 1280×720 and 1440×900 laptop viewports; targeted regression for navigation/focus. The 320/390px phone checks move to T009.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T007, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **passed** in [F03 acceptance](../evidence/F03-acceptance.md), including native reload/restart and reviewed keyboard regressions. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop cosmetic expansion once the approved journeys pass; separate newly requested design work.
