# Atlas agent guide and project roadmap

## Active checkpoint

- **State:** S01 complete on 2026-10-25: T001–T005 passed with native-build, safe-copy data and review evidence.
- **Active:** Owner priority: Windows desktop first. T006–T008 map/lifecycle, T032 guard, T033 SSH verification and **T014/T015 protected desktop sessions/read-only Files passed** on 2026-10-26. Use server workspace → Files → Open file session. Compact phone composition/adapters remain ready T009; S02–S04 stay open. Paired coordinator, mutations, terminal, sudo and transfers remain unavailable. Original Python data is unchanged.
- **Latest:** [T014/T015 proof](evidence/F06-F08-desktop-acceptance.md): actual system SFTP/native browse/stat/lock/invalidation/expiry/cleanup, resolved independent review, 38 ordinary Rust + 1 real-SFTP integration + 4 client tests and SSH/map regressions. Sessions idle after five minutes and expire after 30; leaving Files locks them. Reuse [D017 phasing](discovery/D017-desktop-feature-entry.md), [D018 lifecycle](discovery/D018-sftp-process-and-page-lifecycle.md) and [OPERATIONS.md](OPERATIONS.md). Next desktop file slice is T016; declare its authorised phase explicitly instead of weakening the full S03 gate. Phone/paired/fresh-sudo proof remains intact.
- **Core target:** native Windows and Android, Windows-hosted coordinator, full phone controls over LAN/existing VPN.
- **Budget target:** two five-hour usage windows; a prioritisation target, not a quota or elapsed-time guarantee.
- **Baseline:** A01 Python/SQLite/browser work is historical, with evidence in [IMPLEMENTATION.md](../IMPLEMENTATION.md). It does not complete new Rust/React/native tickets.
- **Tracker:** 33 tickets / 14 features / 8 scopes: 28 core and 5 deferred tickets. Done=12, ready=1 (T009), todo=15, deferred=5. Feature-packet ticket metadata is the sole status authority. Desktop scoped entry changes scheduling only; product scope is unchanged.

## Read protocol

1. Read this guide and [SCOPE.md](SCOPE.md). Consult [STACK.md](../STACK.md) for stack/access detail not in the active packet.
2. Open the selected scope and one feature packet; then its dependency evidence and relevant discoveries/source files.
3. Do not reread the whole tracker or transcript every turn. [WORKFLOW.md](../WORKFLOW.md) supplies chat continuity if needed.
4. Resume from the checkpoint/evidence, not a fresh investigation. The recovered transcript is in the parent project folder; its product decisions are already extracted.

## Roadmap

| Order | Scope | Feature packets | Tickets | Gate / window |
| --- | --- | --- | --- | --- |
| 1 | [S01 Foundations](scopes/S01-foundations.md) | [F01 environment](features/F01-build-environment.md), [F02 persistence](features/F02-domain-persistence.md) | T001–T005 | Both native targets compile; Rust data parity. Window 1 |
| 2 | [S02 Applications/map](scopes/S02-applications-map.md) | [F03 React map](features/F03-react-map.md), [F04 native shells](features/F04-native-shells.md) | T006–T009, T032–T033 | Approved UI, guard and desktop SSH verification. Window 1 |
| 3 | [S03 Connections](scopes/S03-secure-connections.md) | [F05 pairing](features/F05-device-pairing.md), [F06 SSH](features/F06-ssh-elevation.md) | T010–T014 | Revocation, host trust, session/sudo boundaries. Window 1 |
| 4 | [S04 Files](scopes/S04-files-transfers.md) | [F08 files](features/F08-file-manager.md), [F09 transfers](features/F09-transfers.md) | T015–T018 | End-to-end file journey on both clients. Window 2 |
| 5 | [S05 Operations](scopes/S05-observe-operate.md) | [F07 monitoring](features/F07-monitoring.md), [F10 terminal](features/F10-terminal.md), [F11 services](features/F11-service-operations.md) | T019–T023 | Truthful health and authenticated control. Window 2 |
| 6 | [S06 Release](scopes/S06-core-release.md) | [F12 acceptance](features/F12-release-acceptance.md) | T024–T026 | Installable builds, physical-phone journey, owner acceptance. Window 2 |
| 7 | [S07 Agents](scopes/S07-discovery-agents.md) | [F13 discovery](features/F13-discovery-agents.md) | T027–T029 | Deferred; separate owner authorisation |
| 8 | [S08 Sync](scopes/S08-synchronisation.md) | [F14 rsync](features/F14-rsync.md) | T030–T031 | Deferred; separate owner authorisation |

Use this order by default. S05 depends on S03, so an authorised independent feature can proceed when S04 is externally blocked. Do not enter deferred scopes just to fill time. Old roadmap labels are mapped in [ROADMAP.md](../ROADMAP.md).

## Delivery contract

- The documentation task establishes this tracker only. Once the owner authorises a scope/batch, its listed tickets may proceed in dependency order without repeated approval for routine reversible work; actual permission boundaries still apply.
- Deliver one feature/user journey at a time. State its outcome, ticket boundary, proof and stopping condition before implementation. Capture adjacent discoveries as separate work.
- States: `todo`, `ready`, `in_progress`, `blocked`, `done`, `deferred`. Ready/in-progress tickets require completed dependencies. Only T001 starts ready; deferred tickets require explicit activation.
- Desktop-first phased entry may explicitly declare `scope_entry` mapping a prerequisite scope to completed prerequisite ticket IDs. The checker requires nonempty, unique, real dependency IDs belonging to that scope; ordinary entry still requires the full scope. This changes scheduling only: overall scope/release/phone acceptance and evidence rules remain unchanged. Never use entry metadata as an operation-authority or security exception.
- Completion requires actual ticket acceptance and scope-gate evidence. Do not infer physical-device/owner acceptance from source review, mocks or browser screenshots.
- Credential onboarding, signing-key creation, device installation, firewall changes, deployment and real-host modifications require their applicable authorisations. Inventory grants do not imply privileged access.
- Preserve unrelated edits and data. Commit coherent completed features locally after proof; do not automatically push, publish stores or migrate infrastructure.

## Mise/toolchain command contract

T001 established project-local `mise.toml`, pins and tasks. [BUILDING.md](../BUILDING.md) records exact versions and prerequisites; the tasks below are implemented, with native build acceptance tracked separately in T002:

| Command | Purpose |
| --- | --- |
| `mise run doctor` | Effective tools, Windows/Android prerequisites and controlled test fixture availability |
| `mise run deps` | Locked dependency installation; rerun only when inputs change |
| `mise run tracker:check` | Tracker validation |
| `mise run dev:desktop` / `mise run dev:android` | Correct native development target |
| `mise run check` | Non-rewriting type/static checks |
| `mise run test:feature -- <feature-id>` | Smallest mapped meaningful feature checks |
| `mise run test:integration` | Shared storage/auth/transport integration |
| `mise run build:desktop` / `mise run build:android` | Reproducible release artifacts |

Use `mise run exec -- ...` for one-off commands that launch children; [D007](discovery/D007-native-setup-boundaries.md) records the observed Windows PATH issue. Resolve latest stable Node Current, Rust, pnpm and compatible dependencies once, then commit exact tool pins and package/Cargo lockfiles. Use the MSVC Rust host and needed Android targets without changing the global GNU default. Select JDK/SDK/NDK supported by the chosen stable Tauri/Gradle; record exceptions, never downgrade silently. Do not upgrade working dependencies mid-feature. Keep private/machine-specific values in ignored local configuration.

Before T001, the documentation checker works with an existing Python 3.11+: `python agent/check_tracker.py`. Using an already available interpreter for this check does not authorise new toolchain installation or application implementation.

## Efficient testing and refinement

- Documentation: tracker/links/dependencies/status checks, diff review; no app rebuild.
- Ticket: changed behavior and meaningful failure cases; avoid tests that mirror implementation details.
- Feature: one controlled end-to-end journey across its tickets, with shared fixtures and reused valid evidence.
- High-risk boundaries: obtain bounded independent review when available for F02 migration and F05/F06 auth/elevation. Provide only the changed slice, acceptance and evidence; no default multi-agent implementation fan-out.
- Scope: broader regression only when shared contracts/storage/auth changed. Release: actual Windows and physical Android proof, not a phone-size browser alone.
- Reuse pnpm/Cargo/Gradle caches and incremental builds. Start setup/downloads early; avoid repeated installations, entire-tree searches or full-suite loops.
- Make one focused repair attempt for a reproducible failure. If it fails again, record reproduction, hypothesis and blocker before widening investigation. Distinguish product failure, setup failure, approval denial and device/browser unavailability.

## Two-window pacing and continuity

Window 1 targets S01–S03; Window 2 targets S04–S06 and reserves roughly its final quarter for integration, packaging and device acceptance. Reassess after the first native builds and S03. If the budget is insufficient, state remaining acceptance work rather than silently cutting scope. Do not infer exact tokens from elapsed time or assume every account limit resets together.

At feature completion and before a context/usage boundary, update [CHECKPOINT.md](evidence/CHECKPOINT.md): revision, actual tool versions, active scope/feature/ticket, proof links, blockers, next action and verified mise commands. Keep it compact; do not paste full transcripts or logs.

## Discovery and closure

Use [discovery/README.md](discovery/README.md) for meaningful new facts and [evidence/README.md](evidence/README.md) for acceptance records. Never include passwords, keys, pairing secrets or raw protected output. Tick acceptance only when proven; attach an evidence file in ticket metadata, validate the tracker and record the checkpoint. A commit alone is not acceptance evidence.
