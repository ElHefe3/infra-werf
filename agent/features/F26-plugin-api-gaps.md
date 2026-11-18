# F26 — Deferred plugin API gaps found by the reference plugin

<!-- atlas-feature: {"id":"F26","scope":"S13","depends_on":["F25"],"deferred":true} -->

## Work packet

**Outcome:** The plugin API can express what the reference plugin (T051) needed and had to do without.

**User journey:** A plugin author writes the backup overview they actually wanted: every backup on every server, with its next run and its user-scope workers, in one place.

**Scope:** [S13](../scopes/S13-plugin-api-next.md). **Feature prerequisites:** [F25](F25-open-source-readiness.md).

Read [SCOPE.md](../SCOPE.md) for access boundaries; use [AGENT.md](../AGENT.md) for execution/proof rules. Ticket dependencies, not numeric ordering, control implementation. Feature completion is derived from its tickets and scope gate, not independently marked done.

Found while building `examples/systemd-backups`; see [T051 acceptance](../evidence/T051-acceptance.md).

## T054 — Patterns in pack arguments

<!-- atlas-ticket: {"id":"T054","feature":"F26","status":"deferred","depends_on":["T051"],"evidence":null} -->

### Outcome and included work

A pack argument may hold `*`, `?` and `[...]`, which programs such as `systemctl` and `journalctl` match themselves. Today `systemctl list-units 'lab-*-backup.service'` is refused as an unsafe argument, so the reference plugin lists every failing service instead of its backups.

### Exclusions

Anything that would let a shell or the program rule be bypassed.

### Implementation and contracts

No shell reads pack arguments, so these characters reach the program as text. Check that no program in common use expands them into something the owner did not read, add them to the safe set, and show them plainly on the plugin's page. A new plugin API version.

**Ticket dependencies:** [T051](F25-open-source-readiness.md).

### Acceptance criteria

- [ ] A pack can list `lab-*-backup.service` units; the reference plugin's failing list shows only backups.
- [ ] Manifest tests cover the new characters; older API versions still refuse them.

### Verification

Manifest tests; the reference plugin's native suite.

### Evidence and closure

None yet.

### Stop conditions

Stop if any program in the allowed set would treat a pattern as more than a match.

## T055 — Composed and user-scope parameters

<!-- atlas-ticket: {"id":"T055","feature":"F26","status":"deferred","depends_on":["T051"],"evidence":null} -->

### Outcome and included work

Two parameter gaps:

- A parameter fills only a whole argument, so a pack cannot read the timer of a chosen service (`lab-a-backup.timer` from `lab-a-backup.service`) and cannot show a backup's next run.
- Parameters offer only system-scope services and containers. A backup worker running as a user service, as one on the owner's lab does, cannot be chosen.

### Exclusions

Free text from the owner or the plugin as a parameter value.

### Implementation and contracts

Allow a parameter inside an argument with fixed text around it (`--unit={unit}`), and a derived form of a unit name (its stem). Add `user_service` as a parameter source. The value still comes only from discovery, and must stay a safe argument after composition. A new plugin API version.

**Ticket dependencies:** [T051](F25-open-source-readiness.md).

### Acceptance criteria

- [ ] The reference plugin shows each backup's next run.
- [ ] A user-scope service can be chosen and is checked against what the session discovered.

### Verification

Manifest and pack tests; native suite.

### Evidence and closure

None yet.

### Stop conditions

Stop if composition could produce an argument the owner did not see.

## T056 — A result per workload

<!-- atlas-ticket: {"id":"T056","feature":"F26","status":"deferred","depends_on":["T051"],"evidence":null} -->

### Outcome and included work

InfraWerf keeps only the latest result of each command, whatever workload it ran for. Running "Last result" for three backups leaves one, so the Plugin data tab cannot show all backups at once, and the reference plugin's job has to report them in its log instead.

### Exclusions

Saving results to disk (pack results stay in memory).

### Implementation and contracts

Keep the latest result per command and parameter value, within a bound, and show them as a list. A change to the interface and to what a job's `run_command` answers, within the current API version if additive.

**Ticket dependencies:** [T051](F25-open-source-readiness.md).

### Acceptance criteria

- [ ] After the reference plugin's job runs, the Plugin data tab shows each backup's last result.
- [ ] The bound and the in-memory rule hold.

### Verification

Pack tests; native suite.

### Evidence and closure

None yet.

### Stop conditions

None beyond the scope's.

## T057 — Job findings and an overview across servers

<!-- atlas-ticket: {"id":"T057","feature":"F26","status":"deferred","depends_on":["T051","T056"],"evidence":null} -->

### Outcome and included work

Two gaps:

- A job can only succeed or fail. A backup check that works and finds two failed backups shows "Finished", and the server's card says nothing. A failing exit code would be wrong too: three in a row pause the schedule.
- Every plugin point is per server. "Which backups failed, anywhere?" needs one place across servers, and the ticket's own working assumption was an overview across servers.

### Exclusions

A plugin-drawn page outside the isolated view model.

### Implementation and contracts

Let a job's result carry findings (attention, with a short reason) that a badge can show and the job list can mark, separate from success or failure. Add an overview across the servers where the owner allowed the plugin, as a view or a built-in list of findings. A new plugin API version.

**Ticket dependencies:** [T051](F25-open-source-readiness.md), [T056](F26-plugin-api-gaps.md).

### Acceptance criteria

- [ ] The reference plugin's failed backups mark the server's card and its job, without pausing the schedule.
- [ ] One place lists failed backups across every allowed server.

### Verification

Core and native tests; owner review of the overview.

### Evidence and closure

None yet.

### Stop conditions

Stop if an overview would show a server's data where the plugin is not allowed.

## T058 — Clear errors for a module's command

<!-- atlas-ticket: {"id":"T058","feature":"F26","status":"deferred","depends_on":["T051"],"evidence":null} -->

### Outcome and included work

When a job's `run_command` fails, the module always hears `not_granted`, whether the plugin lacks permission, the parameter is not a discovered workload (while building the reference plugin, a name was passed where an ID is expected), or the command does not exist. The module, and its author, cannot tell which.

### Exclusions

Telling a module anything about the server it is not allowed to read.

### Implementation and contracts

Answer `not_granted`, `unknown_workload`, `unknown_command` and `failed` separately. Documented in [plugin logic](../../docs/plugin-api/logic.md). Additive, within sandbox contract 1.

**Ticket dependencies:** [T051](F25-open-source-readiness.md).

### Acceptance criteria

- [ ] Each case answers its own code; a sandbox test shows all four.

### Verification

Core tests.

### Evidence and closure

None yet.

### Stop conditions

None beyond the scope's.
