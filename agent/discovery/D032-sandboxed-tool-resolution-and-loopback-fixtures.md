# D032 — Sandboxed tool resolution and loopback fixtures

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T022; desktop proof workflow

## Finding

This workspace's file/network sandbox can make a pinned runtime appear unavailable even when the exact version is already installed. `mise run` attempted to resolve/install every tool in `mise.toml`; remote version lookups failed, and the Rust updater could not write its temporary file under the user profile outside the writable workspace. Podman's local WSL socket was also blocked when reached from the default sandbox.

## Evidence

On 2026-10-26, `mise run exec -- cargo fmt --all` failed during tool resolution with DNS/network errors and a Rustup temp-file access denial. The already-installed Rust 1.99.1 toolchain, Node 26.12.1 and pnpm 12.15.0 ran directly from their exact installation paths; Rust formatting/tests, contract generation, TypeScript checks and Vite build then succeeded. `podman --connection podman-machine-default info` failed with a local socket permission error in the default sandbox. The bounded image rebuild for the disposable loopback SSH fixture succeeded when run with the authorized elevated local execution.

## Consequence

Before treating a sandbox runtime error as a missing install, inspect the exact locally installed pinned binary and use it without triggering an installer. Keep repository outputs inside the workspace. Native fixture tests that use the local Podman socket require the approved local execution boundary; they continue to target only disposable loopback services.

## Follow-up

If this recurs, report which boundary failed (tool download, profile temp/cache write, or loopback socket) and keep independent code checks moving. Do not switch tool versions or change global sandbox permissions to work around it.
