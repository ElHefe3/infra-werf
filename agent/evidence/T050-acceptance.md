# T050 acceptance — licence, public repository and contribution rules

Date: 2026-11-18. Feature: F25 / S12. Ticket: T050.
Revision: 71cbcae
Result: three of four criteria met; publication awaits the owner's explicit approval

Branch `feature/s12-extensibility`. Nothing was pushed anywhere. The public copy described below exists only on the owner's PC.

## Decisions

Owner decision [D062](../discovery/D062-licence-and-public-repository.md):

- **Licence.** InfraWerf is licensed MIT OR Apache-2.0. Plugins are their authors' own works.
- **History.** The full history is rewritten into a separate public copy; the private repository is unchanged.
- **Records.** `agent/` is published with lab details replaced.
- **Security reports** go through GitHub private vulnerability reporting.

## The audit

Every file version in history (2,271 objects) and every commit message (90 commits at the start) was scanned by an audit script written for this ticket. The table counts private values, not hits. Values are not repeated here.

| Category | Found | Resolution |
| --- | --- | --- |
| Private keys | none (one planted fake string in a test that proves keys never reach the log) | none needed |
| Tokens and API keys | none | none needed |
| Secret-like assignments | 27 patterns, all variable names, prompts or per-run fixture passwords | none needed |
| The owner's server names | 4, in 10 files of `agent/`, `IMPLEMENTATION.md` and the live-check script | replaced with `lab-server-1`…`4` |
| The owner's SSH account | 1, in 2 files and 5 commits | replaced with `owner` |
| The owner's LAN addresses | 4 (three servers, the PC), in records and two test files | replaced with addresses in `192.168.20.0/24` |
| The owner's Windows user name and paths | in 5 records | replaced with `owner` |
| Location and initials | in the two Python-prototype pages | replaced |
| Services the owner runs, and other lab machines | 2 machine names, 6 service names and an app's host name, in 4 files | replaced or reworded |
| A project path in a commit message | 1 | reworded |
| Commit authors | one GitHub no-reply address | kept: the owner publishes under that identity |
| Images | one review screenshot, of a test fixture only | kept |
| Domains | only public projects, documentation and `example` names | none needed |
| Dependency licences | all permissive (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0, a few MPL-2.0); no GPL | none needed |

The owner's terms and their placeholders are in `.public-terms.json`, which Git ignores and which never leaves the PC.

Lab details still in history cannot be fixed by changing the tree. So:

1. The tree was scrubbed and committed on the branch.
2. `scripts/public_repo.py copy` rewrote the branch's full history into a separate repository (`git fast-export` → replacements in every file version and message → `git fast-import`).
3. The copy's history was checked.

## Results on the public copy

Made from `feature/s12-extensibility` at 71cbcae. The copy is the dry run; the real one is made from `master` at publication.

| Check | Result |
| --- | --- |
| Commits rewritten | 93; final tree byte-identical to the private branch (same tree hash `34c8295`) |
| `public_repo.py check` on its tree and full history | 0 problems |
| The audit script on its history | no private key, token, owner path, owner address or owner term; only placeholders, test values and the GitHub handle as author |
| A raw search of its full history and messages for every owner term | 0 matches |
| A clean clone, built from `CONTRIBUTING.md` alone | `mise trust`, `mise install`, `mise run deps`, `mise run check` and `mise run smoke:desktop` all passed; the debug executable was built |

The clean clone was built on this PC, whose tool installs and package caches already existed. It shows that the public documents and repository are sufficient. It does not show what a first-time install downloads.

## Documents

| Document | Content |
| --- | --- |
| [LICENSE-MIT](../../LICENSE-MIT) and [LICENSE-APACHE](../../LICENSE-APACHE) | The licence texts. `Cargo.toml` and `package.json` name the licence. |
| [CONTRIBUTING.md](../../CONTRIBUTING.md) | How to build; what to run before a pull request; the safety rules; contribution licensing. |
| [SECURITY.md](../../SECURITY.md) | How to report privately, scope, testing rules, supported versions. |
| [docs/COMPATIBILITY.md](../../docs/COMPATIBILITY.md) | The versioned interfaces and their rules, what is not an interface, and the plugin licence expectation. |
| [README.md](../../README.md) | A "Start here" section for newcomers. |
| [PUBLISHING.md](../PUBLISHING.md) | The owner's publication steps. |

`scripts/public_repo.py check` is also a contributor check. The live-connection script no longer names the owner's hosts; it reads approved hosts from `ATLAS_LIVE_APPROVED_HOSTS`.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| The licence decision is recorded and applied | D062; licence files; licence fields in both crates and `package.json`; README, CONTRIBUTING and COMPATIBILITY state it. |
| A secrets and private-data audit of the tree and history is recorded with its findings resolved | The audit above. Every finding was replaced, reworded or judged not private. The rewritten public copy has 0 problems by two independent scans and a raw search. |
| Contribution, security-reporting and API-compatibility documents exist | CONTRIBUTING.md, SECURITY.md, docs/COMPATIBILITY.md, linked from the README. |
| The owner has approved publication explicitly | **Not yet.** Asked on 2026-11-18. |

## Notes

- **Evidence hashes.** Commit hashes cited in evidence refer to the private history; the public copy's hashes differ.
- **Publishing from `master`.** The S12 work is on the feature branch and reaches `master` only when the feature set is done. Publication follows [PUBLISHING.md](../PUBLISHING.md) from `master` then. The checker and the copy are deterministic, so the dry run here shows what that will produce.
- **Review.** No separate review pass was run.
