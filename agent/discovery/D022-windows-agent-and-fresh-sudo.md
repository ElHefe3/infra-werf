# D022 — Prove Windows agent use and preserve fresh sudo semantics

Date: 2026-10-26 · T012/T013

An authentication success with an IdentityFile is not evidence of agent signing. The disposable local Windows agent proof disables identity files, advertises one generated key over a random named pipe, counts its real signature request and observes all connections closed. This proves Atlas preserves the system client's configured agent path without modifying the owner's service or moving owner keys. See [T012 acceptance](../evidence/T012-acceptance.md).

The Windows OpenSSH parser accepts forward-slash named-pipe configuration; unescaped backslashes can be consumed by config parsing. Use a fixture-specific `//./pipe/...` path rather than changing owner configuration. Primary project report: [Win32-OpenSSH issue1949](https://github.com/PowerShell/Win32-OpenSSH/issues/1949).

For the upcoming elevated action flow, sudo's `-k` with a command or password-requiring option ignores cached credentials and does not update them. Standalone `sudo -k` has different invalidation behavior; `sudo -K` removes all cached credentials and is excluded. A fresh validation must observe its own nonce prompt and successful password check; a no-prompt/NOPASSWD validation cannot stand in for fresh authentication. Reuse a password only inside that single approved action, then destroy the active flow. Never change sudoers/global timestamp policy or infer sudo completion from SSH authentication.

Primary semantics: [sudo project's manual source](https://raw.githubusercontent.com/sudo-project/sudo/main/docs/sudo.man.in), reset-timestamp/validate sections. T013 still requires implementation, fixture proof and bounded review; this discovery is not an implementation claim.
