# D035 — Workload control daemon identity and uncertain-result recovery

Date: 2026-10-26

## Observed

- Docker discovery and inspection run as the authenticated SSH account, while system Docker control runs with fresh sudo. `DOCKER_HOST`, `DOCKER_CONTEXT` and TLS variables can therefore select a different daemon for the two identities. Every system-Docker path must clear those overrides and pin `/var/run/docker.sock`.
- A successful systemd `Type=oneshot` start or restart normally ends in `inactive`; `Result=success` and the unit type distinguish completed work from a failed start. Report the actual stopped state alongside the successful command result.
- A dropped SSH response after dispatch leaves the service state uncertain. Clear the protected session and results, preserve a generic warning outside the cleared view, and require reconnect/inspection before retry. The same warning must survive when an in-flight confirmation becomes stale because the user locks or the session closes; discard its receipt and reset pending UI state.
- When the Tauri build overrides `beforeBuildCommand` to skip frontend work, run the Vite production build first or the native app will load stale `dist/` assets.

## Applied

T023 pins the same system Docker socket for inventory, inspection/logs and control; parses successful oneshot observations; locks on unknown outcomes; and retains a non-sensitive reconnect warning through explicit Lock, session loss and stale IPC completion. Native coverage verifies the lost response and subsequent observed service state. See [T023 acceptance](../evidence/T023-acceptance.md) and [S05 acceptance](../evidence/S05-acceptance.md).

## Boundaries

The proof used only disposable Linux/OpenSSH containers with loopback SSH. It did not connect to production hosts. The unfiltered Rust library suite remained incomplete because unrelated local HTTP health tests hung under the Windows test environment; the focused workload test filter passed.
