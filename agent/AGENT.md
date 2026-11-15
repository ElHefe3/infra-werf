# InfraWerf agent guide and project roadmap

## Active checkpoint

- **Requested next increment:** dynamic live monitoring and server lifecycle; see [LIVE-MONITORING.md](LIVE-MONITORING.md). Servers may appear, disappear, return or be retired. Plan prominent current health, meaningful bounded history, identity reconciliation and archive/ignore/delete actions. Runtime while the PC is off and specific collector deployments remain undecided; no server installation or server 1 contact is presumed.

- **Current identity:** InfraWerf 0.1.2. The owner's reported inventory/Terminal errors are repaired and the old Atlas program/shortcuts removed. See [desktop retest](evidence/DESKTOP-RETEST-acceptance.md), [D046](discovery/D046-desktop-retest-and-ssh-compatibility.md) and the [brand guide](BRAND.md). All ten native suites, 72 Rust tests, nine UI checks, 14 SSH cases and the real protected-operation fixtures pass; live inventory and Terminal pass on 02–04. The owner data remains unchanged. The ordinary launcher opens the registered release. Core acceptance below retains its original revisions.

- **State:** S01 complete on 2026-10-25: T001–T005 passed with native-build, safe-copy data and review evidence.
- **Active:** Desktop scopes S01–S06 are complete. T026 owner acceptance is recorded in [T026 acceptance](evidence/T026-acceptance.md); [S06 acceptance](evidence/S06-acceptance.md) closes the core release. The installed Windows desktop is primary; the Python reference and native recovery snapshot remain fallbacks. Mobile is future S09; S07/S08 remain deferred.
- **Latest:** [S04 acceptance](evidence/S04-acceptance.md) closes file browse/edit/transfer/protected placement. [T018 proof](evidence/T018-acceptance.md) covers isolated real SSH/SFTP/sudo, cleanup/expiry, Windows build, native elevation regression and independent review; [D027](discovery/D027-protected-stage-session-lifecycle.md) records the staging lifecycle/recovery boundary. Reuse [D021](discovery/D021-transfer-publication-and-build-cache.md) and [OPERATIONS.md](OPERATIONS.md). T012 passed 14 real OpenSSH cases and actual native agent signing ([evidence](evidence/T012-acceptance.md)). [T013](evidence/T013-progress.md) passed on the native Windows app; [S03 proof](evidence/S03-desktop-acceptance.md) closes that gate. [T019](evidence/T019-acceptance.md), [T020](evidence/T020-acceptance.md), [T021](evidence/T021-acceptance.md), [T022](evidence/T022-acceptance.md), [T023](evidence/T023-acceptance.md), [T024](evidence/T024-acceptance.md), [T025](evidence/T025-acceptance.md) and [T026](evidence/T026-acceptance.md) record Windows proof. [D042](discovery/D042-existing-native-store-and-recovery.md), [D043](discovery/D043-current-ssh-agent-has-no-loaded-identities.md) and [D044](discovery/D044-installed-app-review-and-ssh.md) record T026 data and Windows review findings.
- [D032](discovery/D032-sandboxed-tool-resolution-and-loopback-fixtures.md) records the mise tool-resolution and Podman loopback sandbox boundaries encountered in T022.
- [D033](discovery/D033-workload-identity-binds-ssh-profile.md) and [D034](discovery/D034-utf8-safe-log-bounds.md) record the review-driven SSH profile binding and UTF-8-safe log cap requirements for workload operations.
- **Core target:** Windows desktop 100% by the owner’s explicit 2026-10-26 decision. Mobile/coordinator is future S09; agents/rsync stay deferred. Run all remaining desktop batches sequentially without asking between batches; retain proportional proof/review and actual permission boundaries.
- **Budget target:** two five-hour usage windows; a prioritisation target, not a quota or elapsed-time guarantee.
- **Baseline:** A01 Python/SQLite/browser work is historical, with evidence in [IMPLEMENTATION.md](../IMPLEMENTATION.md). It does not complete new Rust/React/native tickets.
- **Tracker:** 53 tickets / 25 features / 12 scopes: 45 core and 8 deferred tickets. Done=31, in progress=2, todo=12, deferred=8. Feature-packet ticket metadata is the sole status authority. S10/F16/T034 is the authorised dynamic desktop extension; mobile and server-agent deployment remain deferred.

## Read protocol

1. Read this guide and [SCOPE.md](SCOPE.md). Consult [STACK.md](../STACK.md) for stack/access detail not in the active packet.
2. Open the selected scope and one feature packet; then its dependency evidence and relevant discoveries/source files.
3. Do not reread the whole tracker or transcript every turn. [WORKFLOW.md](../WORKFLOW.md) supplies chat continuity if needed.
4. Resume from the checkpoint/evidence, not a fresh investigation. The recovered transcript is in the parent project folder; its product decisions are already extracted.

## Roadmap

| Order | Scope | Feature packets | Tickets | Gate / window |
| --- | --- | --- | --- | --- |
| 1 | [S01 Foundations](scopes/S01-foundations.md) | [F01 environment](features/F01-build-environment.md), [F02 persistence](features/F02-domain-persistence.md) | T001–T005 | Pinned foundation builds and Rust data parity. Window 1 |
| 2 | [S02 Applications/map](scopes/S02-applications-map.md) | [F03 React map](features/F03-react-map.md), [F04 native shells](features/F04-native-shells.md) | T006–T008, T032–T033 | Approved UI, guard and desktop SSH verification. Window 1 |
| 3 | [S03 Connections](scopes/S03-secure-connections.md) | [F06 SSH](features/F06-ssh-elevation.md) | T012–T014 | Host trust, session/sudo boundaries; pairing is S09. Window 1 |
| 4 | [S04 Files](scopes/S04-files-transfers.md) | [F08 files](features/F08-file-manager.md), [F09 transfers](features/F09-transfers.md) | T015–T018 | End-to-end Windows file journey. Window 2 |
| 5 | [S05 Operations](scopes/S05-observe-operate.md) | [F07 monitoring](features/F07-monitoring.md), [F10 terminal](features/F10-terminal.md), [F11 services](features/F11-service-operations.md) | T019–T023 | Truthful health and authenticated control. Window 2 |
| 6 | [S06 Release](scopes/S06-core-release.md) | [F12 acceptance](features/F12-release-acceptance.md) | T024–T026 | Windows installer, recovery, desktop owner acceptance. Window 2 |
| 7 | [S07 Agents](scopes/S07-discovery-agents.md) | [F13 discovery](features/F13-discovery-agents.md) | T027–T029 | Deferred; separate owner authorisation |
| 8 | [S08 Sync](scopes/S08-synchronisation.md) | [F14 rsync](features/F14-rsync.md) | T030–T031 | Deferred; separate owner authorisation |
| 10 | [S10 Live desktop](scopes/S10-live-desktop.md) | [F16 monitoring and lifecycle](features/F16-live-desktop.md) | T034 | Authorised desktop extension after S06 |
| 11 | [S11 Remote applications](scopes/S11-remote-applications.md) | [F17 remote applications](features/F17-remote-applications.md) | T035–T036 | Owner-activated 2026-11-08 (D051); loopback forwards only |
| 12 | [S12 Extensibility](scopes/S12-extensibility.md) | [F18 extension points](features/F18-extension-points.md), [F19 plugin API](features/F19-plugin-api-manifest.md), [F20 permissions](features/F20-plugin-permissions.md), [F21 packs](features/F21-declarative-packs.md), [F22 install](features/F22-plugin-install-integrity.md), [F23 views](features/F23-plugin-views.md), [F24 logic/automation](features/F24-plugin-logic-automation.md), [F25 open source](features/F25-open-source-readiness.md) | T037–T052 | Owner-activated 2026-11-12 (D053); on branch `feature/s12-extensibility` until the feature set is done |

[S09 Future mobile](scopes/S09-future-mobile.md) retains T009–T011, Android adapters, pairing and phone acceptance for separate activation after desktop completion.

Use this order by default. S05 depends on S03, so an authorised independent feature can proceed when S04 is externally blocked. Do not enter deferred scopes just to fill time. Old roadmap labels are mapped in [ROADMAP.md](../ROADMAP.md).

## Delivery contract

- The documentation task establishes this tracker only. Once the owner authorises a scope/batch, its listed tickets may proceed in dependency order without repeated approval for routine reversible work; actual permission boundaries still apply.
- Deliver one feature/user journey at a time. State its outcome, ticket boundary, proof and stopping condition before implementation. Capture adjacent discoveries as separate work.
- States: `todo`, `ready`, `in_progress`, `blocked`, `done`, `deferred`. Ready/in-progress tickets require completed dependencies. Only T001 starts ready; deferred tickets require explicit activation.
- Desktop-first phased entry may explicitly declare `scope_entry` mapping a prerequisite scope to completed prerequisite ticket IDs. The checker requires nonempty, unique, real dependency IDs belonging to that scope; ordinary entry still requires the full scope. This changes scheduling only: overall scope/release/phone acceptance and evidence rules remain unchanged. Never use entry metadata as an operation-authority or security exception.
- Completion requires actual ticket acceptance and scope-gate evidence. For this desktop release, prove native behavior on the actual Windows PC and record owner acceptance; do not infer it from source review, mocks or browser screenshots. Phone/Android acceptance remains in future S09.
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
- Scope: broader regression only when shared contracts/storage/auth changed. Release: actual Windows build/install and owner acceptance; phone/Android proof remains in future S09.
- Reuse pnpm/Cargo/Gradle caches and incremental builds. Start setup/downloads early; avoid repeated installations, entire-tree searches or full-suite loops.
- Make one focused repair attempt for a reproducible failure. If it fails again, record reproduction, hypothesis and blocker before widening investigation. Distinguish product failure, setup failure, approval denial and device/browser unavailability.

## Two-window pacing and continuity

Window 1 targets S01–S03; Window 2 targets S04–S06 and reserves roughly its final quarter for integration, Windows packaging and owner acceptance. Mobile/device acceptance remains future S09. Reassess after the first native builds and S03. If the budget is insufficient, state remaining acceptance work rather than silently cutting scope. Do not infer exact tokens from elapsed time or assume every account limit resets together.

At feature completion and before a context/usage boundary, update [CHECKPOINT.md](evidence/CHECKPOINT.md): revision, actual tool versions, active scope/feature/ticket, proof links, blockers, next action and verified mise commands. Keep it compact; do not paste full transcripts or logs.

## Discovery and closure

Use [discovery/README.md](discovery/README.md) for meaningful new facts and [evidence/README.md](evidence/README.md) for acceptance records. Never include passwords, keys, pairing secrets or raw protected output. Tick acceptance only when proven; attach an evidence file in ticket metadata, validate the tracker and record the checkpoint. A commit alone is not acceptance evidence.
