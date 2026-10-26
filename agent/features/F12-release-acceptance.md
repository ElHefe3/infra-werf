# F12 — An installable core release with cross-feature proof, data recovery and desktop owner acceptance.

<!-- atlas-feature: {"id":"F12","scope":"S06","depends_on":["F09","F07","F10","F11"],"deferred":false} -->

Mobile/APK/pairing/physical-phone requirements are retained in [future S09](../scopes/S09-future-mobile.md); they do not gate desktop completion.

Current owner decision (2026-10-26): desktop is the complete release target. Phone/coordinator/APK requirements are preserved in [S09](../scopes/S09-future-mobile.md), separately deferred. All desktop batches are authorised sequentially.

## Work packet

**Outcome:** An installable core release with cross-feature proof, data recovery and desktop owner acceptance.

**User journey:** Migrate safe copy → install Windows app → authenticate/control/transfer → restart/recover → owner accepts cutover.

**Scope:** [S06](../scopes/S06-core-release.md). **Feature prerequisites:** [F09](F09-transfers.md), [F07](F07-monitoring.md), [F10](F10-terminal.md), [F11](F11-service-operations.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D002](../discovery/D002-rust-host.md), [D003](../discovery/D003-android-prerequisites.md), [D004](../discovery/D004-migration-baseline.md).

## T024 — Verify integrated core and recovery

<!-- atlas-ticket: {"id":"T024","feature":"F12","status":"done","depends_on":["T018","T020","T021","T023"],"evidence":"../evidence/T024-acceptance.md"} -->

### Outcome and included work

Prove the combined data/auth/transport/operation boundaries and migration rollback before packaging.

### Exclusions

Repeated exhaustive component suites, performance campaigns or release acceptance inferred from automated tests alone.

### Implementation and contracts

Reuse passed feature evidence unless changed; run broader tests only for shared paths and core journeys. Exercise restart/revocation/interrupted work across features using synthetic fixtures and a safe migrated data copy.

**Ticket dependencies:** [T018](F09-transfers.md), [T020](F07-monitoring.md), [T021](F10-terminal.md), [T023](F11-service-operations.md).

### Acceptance criteria

- [x] All core feature acceptance has valid evidence and shared contract/storage/auth regression passes.
- [x] Migration/restart/rollback preserve identities/layout/relationships and approved data; no simultaneous legacy/replacement writes occur.
- [x] Failure/revoke/expiry across file, terminal and service flows leaves no credential or protected-result persistence.

### Verification

One targeted integrated regression and recovery run, linked to prior feature proof; investigate only new failures.

### Evidence and closure

See [T024 acceptance](../evidence/T024-acceptance.md) for the tested revision, synthetic migration and rollback, native Windows restart journey, shared regression results, reused cross-feature evidence, limitations and recovery state. Continue with T025 packaging; T026 owner acceptance remains a separate release gate.

### Stop conditions

Retain the Python reference/original data and block packaging/cutover for unresolved critical data/auth failures.

## T025 — Package Windows desktop artifacts

<!-- atlas-ticket: {"id":"T025","feature":"F12","status":"done","depends_on":["T024"],"evidence":"../evidence/T025-acceptance.md"} -->

### Outcome and included work

Produce installable Windows x64 artifacts through pinned mise tasks.

### Exclusions

Google Play/Microsoft Store publication, iOS, public hosting or purchasing signing certificates.

### Implementation and contracts

Use the verified current stable toolchain and lockfiles. Record Windows artifact checksums, signing/unsigned status honestly, and clean install/restart instructions; Android signing remains S09. No paid Windows certificate or store publication is assumed.

**Ticket dependencies:** [T024](F12-release-acceptance.md).

### Acceptance criteria

- [x] Documented mise tasks produce reproducible-configuration Windows installer artifacts.
- [x] Artifact checksums/signature validation and actual install/launch results are recorded; debug artifacts are not labelled release.
- [x] Signing secrets are not in repository/logs, and uninstall/update/data-location behavior is documented and checked.

### Verification

One Windows release build and controlled installation smoke; reuse caches, do not rebuild unchanged artifacts for documentation edits.

### Evidence and closure

See [T025 acceptance](../evidence/T025-acceptance.md) for the release build, checksum/signature status, installed-binary journey, same-version update, uninstall/reinstall, data-preservation boundary and remaining distribution limitations. T026 owner acceptance remains open; see [the checkpoint](../evidence/CHECKPOINT.md).

### Stop conditions

Record signing/installer/device approval blocks explicitly; never embed keys or bypass OS security warnings to manufacture a pass.

## T026 — Complete Windows desktop acceptance and cutover

<!-- atlas-ticket: {"id":"T026","feature":"F12","status":"in_progress","depends_on":["T025"],"evidence":null} -->

### Outcome and included work

Obtain owner acceptance of the native core journeys and perform a documented reversible cutover.

### Exclusions

Assuming acceptance, silently retiring the only fallback, installing agents or starting deferred scopes.

### Implementation and contracts

Use the actual Windows app on the selected LAN/existing VPN. Verify the existing native data copy and its recovery snapshot before cutover; do not overwrite either. Verify strict-trust SSH authentication/lock/reconnect, file round trip/protected placement, terminal, health and scoped control on explicitly approved fixtures/targets. Pairing, phone access and Android journeys remain future S09 work. Document availability/restart/rollback and remaining limits.

**Ticket dependencies:** [T025](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Owner evidence on the actual Windows PC covers the full desktop core, including session/host-trust failures and application lifecycle.
- [ ] Cutover preserves data with verified recovery; reference/original backups are retained until acceptance and retirement is separately deliberate.
- [ ] Roadmap/tickets/evidence/release instructions accurately state what shipped, what is deferred and any unresolved blocker.

### Verification

One concise owner-facing acceptance checklist/journey set; reuse valid technical proof and record unavailable/manual items as open.

### Evidence and closure

The prepared [T026 owner checklist](../evidence/T026-owner-checklist.md) is not acceptance evidence. Record the actual Windows-PC owner observations and cutover decision there, then link a passing acceptance record in ticket metadata. Current owner acceptance: **pending**. Do not close T026 from fixture evidence alone; use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Keep release acceptance open if a required Windows-PC owner check or recovery proof is missing.
