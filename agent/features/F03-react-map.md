# F03 — The approved Atlas experience in shared typed React components, using React Flow for map mechanics.

<!-- atlas-feature: {"id":"F03","scope":"S02","depends_on":["F02"],"deferred":false} -->

## Work packet

**Outcome:** The approved Atlas experience in shared typed React components, using React Flow for map mechanics.

**User journey:** Load persisted map → search/edit/connect → inspect workspace → reload, on laptop and compact phone layouts.

**Scope:** [S02](../scopes/S02-applications-map.md). **Feature prerequisites:** [F02](F02-domain-persistence.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D004](../discovery/D004-migration-baseline.md).

## T006 — Replace handwritten map with React Flow

<!-- atlas-ticket: {"id":"T006","feature":"F03","status":"ready","depends_on":["T003","T004","T005"],"evidence":null} -->

### Outcome and included work

Port real server/relationship editing to React Flow with persistent domain state.

### Exclusions

Visual redesign, automatic infrastructure rewiring or porting obsolete demo behavior.

### Implementation and contracts

Preserve approved server-card styling, IDs and desktop layout. Support custom server nodes, directed labelled relationships, selection/pan/zoom and real persistence. Use compact mobile arrangement without overwriting desktop coordinates.

**Ticket dependencies:** [T003](F02-domain-persistence.md), [T004](F02-domain-persistence.md), [T005](F02-domain-persistence.md).

### Acceptance criteria

- [ ] Add/edit hosts, move desktop nodes and create/remove relationships survive reload/restart.
- [ ] Search/focus/selection and touch pan/zoom work without unintended edits or nested-service identity loss.
- [ ] All displayed state comes from the domain/approved inventory; no simulated successful operations or invented live values.

### Verification

Focused map-model tests and one save/reload journey with a controlled fixture; test touch interactions in the shared UI, leaving native device proof to T026.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T006, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record a layout/contract mismatch before changing preserved identities or flattening owner data.

## T007 — Port shared layouts and server workspace

<!-- atlas-ticket: {"id":"T007","feature":"F03","status":"todo","depends_on":["T006"],"evidence":null} -->

### Outcome and included work

Reproduce approved desktop and separate compact phone interfaces using shared React components.

### Exclusions

Floating-window OS emulation, new themes or claiming browser viewport checks prove native Android.

### Implementation and contracts

Provide inspector plus Files/Services/Terminal/System/Settings/Transfers workspace with tabs. Keep one main app on small screens, reusable status/error components and responsive navigation. Use adapters so platform-specific transport/file handling stays outside presentation.

**Ticket dependencies:** [T006](F03-react-map.md).

### Acceptance criteria

- [ ] Laptop and narrow phone layouts preserve the approved look with readable content and no document overflow.
- [ ] Search and full inventory browsing work with the existing large report shape; loading/offline/empty/error states are truthful.
- [ ] Workspace controls are accessible by keyboard/touch and unimplemented operations are labelled unavailable until their tickets pass.

### Verification

One component/journey set at laptop and 320/390px phone widths; targeted regression for navigation/focus after layout changes.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T007, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop cosmetic expansion once the approved journeys pass; separate newly requested design work.
