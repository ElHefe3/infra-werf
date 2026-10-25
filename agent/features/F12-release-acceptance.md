# F12 — An installable core release with cross-feature proof, data recovery and actual-owner phone acceptance.

<!-- atlas-feature: {"id":"F12","scope":"S06","depends_on":["F09","F07","F10","F11"],"deferred":false} -->

## Work packet

**Outcome:** An installable core release with cross-feature proof, data recovery and actual-owner phone acceptance.

**User journey:** Migrate safe copy → install both apps → pair/control/transfer → restart/recover → owner accepts cutover.

**Scope:** [S06](../scopes/S06-core-release.md). **Feature prerequisites:** [F09](F09-transfers.md), [F07](F07-monitoring.md), [F10](F10-terminal.md), [F11](F11-service-operations.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

**Relevant discoveries:** [D002](../discovery/D002-rust-host.md), [D003](../discovery/D003-android-prerequisites.md), [D004](../discovery/D004-migration-baseline.md).

## T024 — Verify integrated core and recovery

<!-- atlas-ticket: {"id":"T024","feature":"F12","status":"todo","depends_on":["T018","T020","T021","T023"],"evidence":null} -->

### Outcome and included work

Prove the combined data/auth/transport/operation boundaries and migration rollback before packaging.

### Exclusions

Repeated exhaustive component suites, performance campaigns or release acceptance inferred from automated tests alone.

### Implementation and contracts

Reuse passed feature evidence unless changed; run broader tests only for shared paths and core journeys. Exercise restart/revocation/interrupted work across features using synthetic fixtures and a safe migrated data copy.

**Ticket dependencies:** [T018](F09-transfers.md), [T020](F07-monitoring.md), [T021](F10-terminal.md), [T023](F11-service-operations.md).

### Acceptance criteria

- [ ] All core feature acceptance has valid evidence and shared contract/storage/auth regression passes.
- [ ] Migration/restart/rollback preserve identities/layout/relationships and approved data; no simultaneous legacy/replacement writes occur.
- [ ] Failure/revoke/expiry across file, terminal and service flows leaves no credential or protected-result persistence.

### Verification

One targeted integrated regression and recovery run, linked to prior feature proof; investigate only new failures.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T024, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Retain the Python reference/original data and block packaging/cutover for unresolved critical data/auth failures.

## T025 — Package Windows and signed Android artifacts

<!-- atlas-ticket: {"id":"T025","feature":"F12","status":"todo","depends_on":["T024"],"evidence":null} -->

### Outcome and included work

Produce installable Windows x64 and signed sideloadable Android ARM64 artifacts through pinned mise tasks.

### Exclusions

Google Play/Microsoft Store publication, iOS, public hosting or purchasing signing certificates.

### Implementation and contracts

Use the verified current stable toolchain and lockfiles. Keep Android release signing material protected/outside Git with authorised creation/use; record checksums/signature verification and clean install/restart instructions. No paid Windows certificate or store publication is assumed.

**Ticket dependencies:** [T024](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Documented mise tasks produce reproducible-configuration Windows installer and signed Android APK artifacts.
- [ ] Artifact checksums/signature validation and actual install/launch results are recorded; debug artifacts are not labelled release.
- [ ] Signing secrets are not in repository/logs, and uninstall/update/data-location behavior is documented and checked.

### Verification

One release build per target and controlled installation smoke; reuse caches, do not rebuild unchanged artifacts for documentation edits.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T025, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record signing/installer/device approval blocks explicitly; never embed keys or bypass OS security warnings to manufacture a pass.

## T026 — Complete actual-device acceptance and cutover

<!-- atlas-ticket: {"id":"T026","feature":"F12","status":"todo","depends_on":["T025"],"evidence":null} -->

### Outcome and included work

Obtain owner acceptance of the native core journeys and perform a documented reversible cutover.

### Exclusions

Assuming acceptance, silently retiring the only fallback, installing agents or starting deferred scopes.

### Implementation and contracts

Use the actual Android phone and Windows app on the selected LAN/existing VPN. Verify migration, pair/revoke/reconnect, file round trip/protected placement, terminal, health and scoped control on explicitly approved fixtures/targets. Document availability/restart/rollback and remaining limits.

**Ticket dependencies:** [T025](F12-release-acceptance.md).

### Acceptance criteria

- [ ] Owner/device evidence covers full core controls on native Android and Windows, including session/host-trust failures and phone lifecycle.
- [ ] Cutover preserves data with verified recovery; reference/original backups are retained until acceptance and retirement is separately deliberate.
- [ ] Roadmap/tickets/evidence/release instructions accurately state what shipped, what is deferred and any unresolved blocker.

### Verification

One concise owner-facing acceptance checklist/journey set; reuse valid technical proof and record unavailable/manual items as open.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T026, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Keep release acceptance open if a required physical-device/owner check or recovery proof is missing.
