# D006 — An existing SSH host has an unsupported app-server version

Date: 2026-10-25
State: verified
Affected: T029

## Finding

Desktop diagnostics repeatedly reported `codex-app-server-version-unsupported:0.140.0` and `Thread catalog sync failed` for an existing connected SSH host. This is a Codex host-connection error, not an error in Atlas's Python coordinator or the proposed Rust backend.

## Evidence

Scoped read-only desktop log inspection on 25 October 2026 found these messages for the remote SSH host connection around 09:48 UTC. No remote command, installation, update or connection change was performed. The logs do not establish that this separate failure caused the local file-edit rejections or all Android sidebar symptoms.

## Consequence

Do not treat that host as a proven healthy remote development environment or use it as a reason to alter Atlas data/authentication. Keep this environmental issue separate from product bugs and from the PC-first core scope.

## Follow-up

If the owner selects remote development/always-on deployment later, diagnose and update the relevant Codex host connection under a separately approved environment operation. T029 must verify its actual target and tooling rather than inheriting an assumed healthy connection. No remote-host update is part of tracker creation.
