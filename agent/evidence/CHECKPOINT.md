# Atlas restart checkpoint

Date: 2026-10-25
State: documentation/tracker complete and validated; no application ticket implemented.
Revision: obtain with `git log -1 --oneline`; feature proof must name its actual tested revision.

## Working set

Read [AGENT.md](../AGENT.md), [S01](../scopes/S01-foundations.md), then [F01](../features/F01-build-environment.md). Next is **T001**, after authorisation to execute S01/application work.

Core: native Windows/Android, PC-hosted, full phone controls, LAN/existing VPN. Agents/rsync are deferred. No project pins/tasks exist yet; planning observations are not version selections. Preserve the Python reference/database and unrelated work.

## Verification and blockers

Documentation evidence is [DOCS-001](DOCS-001.md): tracker validation, ten rejection fixtures and one positive completion fixture passed. Counts: 31 tickets / 14 features / 8 scopes; ready=1, todo=25, deferred=5. No application feature proof exists yet. Native compilation, controlled SSH fixture and physical phone acceptance are future checks, not confirmed successes or failures.

New workflow/environment findings are [D005](../discovery/D005-edit-approval-observations.md) and [D006](../discovery/D006-separate-ssh-catalog-error.md). The edit-rejection cause remains unproven; no sandbox/approval setting was changed. The separate remote host version error is not an Atlas product failure or permission to update that host.

## Next commands

Verified tracker command: `python agent/check_tracker.py --self-test` using an existing Python 3.11+ interpreter. Proposed mise tasks are T001 deliverables, not verified commands. Start S01/F01/T001 when application work is selected; resolve latest stable tools through mise, then record the exact effective versions and tested setup/check/resume commands here. Preserve the reference database and do not mark legacy A01 proof as native migration completion.
