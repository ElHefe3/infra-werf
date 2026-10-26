# D037 — Bound the Windows migration writer probe without asyncio subprocesses

Date: 2026-10-26<br>
Affected: T024

## Finding

The migration parity script's Windows-only writer-lock check stalled when Python 3.12 used `asyncio.create_subprocess_exec`, including a trivial `cmd.exe` child. The same `atlas-data hold-lock` command worked interactively, and ordinary `subprocess.run` already completed migration/state operations. This is limited to the fixture's async child-process path; it is not evidence of a product storage deadlock.

## Resolution

Use a synchronous `Popen` for the lock fixture and a bounded reader thread for its readiness line, retaining five-second readiness/release limits and guaranteed child cleanup. See [T024](../features/F12-release-acceptance.md) evidence when available.
