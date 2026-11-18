# Security policy

InfraWerf holds the keys to someone's servers: it opens SSH sessions, moves files, runs reviewed commands and lets plugins act within the permissions their owner grants. We take reports about it seriously.

## Reporting a vulnerability

**Please do not open a public issue.** Report privately through GitHub: on the repository's **Security** tab, choose **Report a vulnerability**. Only the maintainers see the report.

Please include:

- what an attacker can do, and what they need first (a malicious plugin, a hostile server, a crafted file, access to the PC);
- the InfraWerf version or commit, and Windows version;
- steps or a proof of concept, using your own test systems.

We will acknowledge a report as soon as we can, tell you whether we can reproduce it, and agree a disclosure date with you. InfraWerf is a volunteer project, so please allow reasonable time for a fix. We credit reporters who want to be credited.

## What is in scope

- The desktop application and its Rust core.
- Everything that stands between a plugin and the owner's servers: the manifest checks, permissions and trust levels, the install and update checks, the view isolation, the WebAssembly sandbox and its host functions, and the job, schedule and sudo rules.
- SSH handling: host-key trust, sessions, file transfers, the terminal, sudo prompts.
- Anything that sends the owner's data off their PC, or keeps a password or key that should not be kept.

Out of scope: a third-party plugin's own bugs (report them to its author, unless InfraWerf's checks failed to contain it), problems that need an attacker to already control the owner's Windows account, and weaknesses in servers InfraWerf connects to.

## Testing rules

Test only against systems you own or are allowed to test. InfraWerf's own suites run against disposable loopback containers (`scripts/linux_fixture.py`); please do the same. Never test against someone else's servers or InfraWerf instance.

## Supported versions

Security fixes go into the latest release and the `main` branch. InfraWerf is pre-1.0, so older releases are not patched.

## Design references

- [Plugin permissions and trust](docs/plugin-api/guide.md#what-the-owner-sees)
- [Plugin views and their isolation](docs/plugin-api/views.md)
- [Plugin logic, the sandbox and jobs](docs/plugin-api/logic.md)
- [Installing and updating plugins](docs/plugin-api/install.md)
- Decision records under `agent/discovery/` and security reviews under `agent/evidence/`
