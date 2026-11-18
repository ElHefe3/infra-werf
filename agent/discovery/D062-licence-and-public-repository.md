# D062 — Licence and the public repository

Date: 2026-11-18. Related: S12 / F25 T050; [D052](D052-plugin-platform-direction.md).

## Owner decisions

Asked on 2026-11-18, after the private-data audit in [T050 acceptance](../evidence/T050-acceptance.md), the owner chose:

1. **Licence: MIT OR Apache-2.0.** InfraWerf's own code is dual-licensed, at the user's option, the Rust ecosystem's usual terms. Every dependency was already permissive (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, a few MPL-2.0 files; no GPL), so the choice needs no exceptions. Contributions are accepted under the same terms (inbound = outbound).
2. **Plugins are their authors' own works.** A plugin talks to InfraWerf only through its manifest, its sandboxed module and its view's message API. Its author licenses it as they choose and names the licence in the manifest's `license` field. Nothing in InfraWerf's licence reaches into a plugin.
3. **Rewrite all history.** The public repository keeps the full commit history, rewritten so that no commit contains the owner's lab details. The private repository and its history are not changed. Rewriting is done into a separate copy by `scripts/public_repo.py`, which takes the owner's private replacements from an ignored file that never leaves this PC.
4. **Publish `agent/` with lab details replaced.** The design record (scopes, features, tickets, discovery notes, evidence) is public, with the owner's server names, SSH account, LAN addresses, Windows user name, location and the services the owner runs replaced by neutral placeholders.
5. **Security reports through GitHub private vulnerability reporting** on the public repository. `SECURITY.md` says so.

## Consequences

- Commit hashes cited in evidence refer to the private history. The public history has different hashes; a note in the public copy's README says so.
- Publishing remains the owner's action. Nothing is pushed to a public remote without the owner's explicit approval, and the public copy is checked again immediately before it is pushed.
- New records must not reintroduce lab details: `scripts/public_repo.py check` fails if the tree contains any of them.
