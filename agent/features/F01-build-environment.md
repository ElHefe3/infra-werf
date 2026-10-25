# F01 — A reproducible latest-stable mise environment that proves Windows and Android feasibility before feature work.

<!-- atlas-feature: {"id":"F01","scope":"S01","depends_on":[],"deferred":false} -->

## Work packet

**Outcome:** A reproducible latest-stable mise environment that proves Windows and Android feasibility before feature work.

**User journey:** Fresh project environment → effective pinned versions → thin native Windows and Android builds.

**Scope:** [S01](../scopes/S01-foundations.md). **Feature prerequisites:** None.

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D001](../discovery/D001-runtime-selection.md), [D002](../discovery/D002-rust-host.md), [D003](../discovery/D003-android-prerequisites.md), [D005](../discovery/D005-edit-approval-observations.md), [D007](../discovery/D007-native-setup-boundaries.md).

## T001 — Pin current tools and establish mise tasks

<!-- atlas-ticket: {"id":"T001","feature":"F01","status":"done","depends_on":[],"evidence":"../evidence/F01-T001.md"} -->

### Outcome and included work

Configure project-local mise with the latest stable Node Current, Rust, pnpm and compatible dependencies; verify effective tools and prerequisites.

### Exclusions

Application behavior, global tool-default changes, native release claims and unrelated environment repair.

### Implementation and contracts

Resolve official stable releases once, select the Windows MSVC Rust host per project, add Android targets, and commit exact pins. Define the task interface in AGENT.md, dependency locks and environment diagnostics. Keep machine paths/secrets in ignored local settings.

**Ticket dependencies:** None.

### Acceptance criteria

- [x] Effective Node/Rust/pnpm versions run through mise and are exact project pins; Node 14 and the global GNU host are not selected accidentally.
- [x] Doctor identifies MSVC/linker/WebView2, JDK/SDK/NDK, signing/device and controlled SSH-fixture readiness; missing items are explicit.
- [x] Required tasks execute the appropriate check or explicitly report that no implementation/test exists yet; no fake pass. Version and compatibility choices are recorded.

### Verification

Run doctor and effective-version checks once; validate task/config parsing. Do not build every feature or repeatedly resolve latest versions.

### Evidence and closure

Tool selection, diagnostics and task checks passed; see [T001 evidence](../evidence/F01-T001.md). Native platform acceptance is tracked separately in T002. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Stop the affected setup on missing licences/permissions or incompatible current releases; record the exact blocker and compatibility evidence.

## T002 — Prove thin Windows and Android native builds

<!-- atlas-ticket: {"id":"T002","feature":"F01","status":"done","depends_on":["T001"],"evidence":"../evidence/F01-acceptance.md"} -->

### Outcome and included work

Create the smallest shared Tauri/React/TypeScript skeleton that compiles for Windows x64 and Android ARM64.

### Exclusions

Complete map migration, release signing/distribution and service credentials.

### Implementation and contracts

Use the pinned toolchain and common frontend. Compile the native shells before implementing domain/UI features; keep host-specific Windows code out of Android builds. Record actual artifacts/commands and cache reuse.

**Ticket dependencies:** [T001](F01-build-environment.md).

### Acceptance criteria

- [x] A thin Windows native application builds and launches.
- [x] An Android ARM64 native artifact builds; emulator/device execution status is recorded separately rather than inferred.
- [x] Re-running the documented mise build commands uses the pinned setup and preserves successful dependency/build caches.

### Verification

One thin native build per platform; smoke launch where available. Physical-phone release acceptance remains T026.

### Evidence and closure

Windows native build/launch, the Android ARM64 debug APK, repeat builds/cache reuse and the focused F01 checks passed. See [F01 acceptance](../evidence/F01-acceptance.md). Android runtime execution and release signing remain separate, untested gates. The historical permission failure is retained in [T002 progress](../evidence/F01-T002-progress.md).

### Stop conditions

After one focused environment repair, capture remaining build failure as a blocker instead of looping through toolchain reinstalls.
