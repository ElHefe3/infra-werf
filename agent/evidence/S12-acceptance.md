# S12 acceptance — extensibility

Date: 2026-11-18. Scope: S12 (F18–F25). Tickets: T037–T053, except T054–T058, which are deferred to S13.
Revision: 1e6ec8c
Result: pass

Branch `feature/s12-extensibility`. Windows 11 x64. Every S12 ticket is done with its own evidence. This record maps the scope's acceptance gate to that evidence and to a final full run on 1e6ec8c.

## Final run on 1e6ec8c

| Check | Result |
| --- | --- |
| `mise run check` | pass (on `d0404f9`; `1e6ec8c` changes only a fixture script and the tracker's counts) |
| `cargo test --workspace --locked -- --test-threads=1` | pass: 159 passed, 2 ignored (need a real SSH fixture), on `d0404f9` |
| UI tests | pass: 38 of 38, on `d0404f9` |
| `check_tracker.py --self-test` | pass |
| `public_repo.py check` | 0 problems |
| Native suites | All 19 passed in one uninterrupted run of `ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe mise run test:desktop:full` on `1e6ec8c`, with a freshly built release and fixture image. An earlier run on `d0404f9` stopped at `packs`: the T051 fixture change had made `systemctl is-system-running` answer in every suite. Its new commands now answer only when the backups suite's marker exists. |

## Gate

| Gate item | Evidence |
| --- | --- |
| Built-in features are delivered through the same extension points plugins use | [T037](T037-acceptance.md): registries for tabs, badges, collectors and operations. [T038](T038-acceptance.md): remote applications as a built-in extension. [T052](T052-acceptance.md): the Settings screen as an extension point. [T053](T053-acceptance.md): the main menu. |
| A plugin can do nothing its manifest did not request and the owner did not grant; enforcement is in the Rust core | [T041](T041-acceptance.md): every capability attempted in every state, admitted only where granted. [T043](T043-acceptance.md): each pack run checked again, and cancelled when refused. [T046](T046-acceptance.md): updates never add grants, and new capabilities need a new review. [T047](T047-acceptance.md): a view reaches InfraWerf only through checked requests. [T048](T048-acceptance.md): each host function maps to one capability, decided by the core. [T049](T049-acceptance.md): each job action is decided by trust, grant, schedule and exception. |
| No plugin can obtain private keys, sudo passwords, a raw SSH session or unrestricted local file access | Packs run in InfraWerf's own session and receive extracted values only ([T043](T043-acceptance.md)). `interactive_terminal` cannot be requested ([T039](T039-acceptance.md)). Views are isolated, confirmed by a separate hostile review ([T047](T047-acceptance.md), [review](T047-isolation-review.md)). Modules have no system interface, no files and no network ([T048](T048-acceptance.md)). The sudo password for a job's action is typed into InfraWerf and is in neither the database, the log nor the job's state ([T049](T049-acceptance.md)). Installs stay inside the plugins folder; traversal archives are refused ([T045](T045-acceptance.md)). |
| The owner can see what each plugin did and disable any plugin immediately | [T042](T042-acceptance.md): the activity log, withdrawing trusted automation, and the kill switch. Views close ([T047](T047-acceptance.md)) and job runs stop ([T049](T049-acceptance.md)) when permissions change; schedules are deleted on disable, revoke or remove. |
| The plugin API is versioned and documented, and a reference plugin built only on it runs on the owner's lab | [T039](T039-acceptance.md), [T040](T040-acceptance.md), [T044](T044-acceptance.md): the versioned manifest, the guide and the reference. The view, sandbox and job contracts are documented with T047–T049. [What InfraWerf keeps stable](../../docs/COMPATIBILITY.md) is from T050. [T051](T051-acceptance.md): the Systemd backups plugin ran on three of the owner's servers and the owner accepted it. |
| Publication as open source is an explicit, recorded owner decision after a private-data audit | [T050](T050-acceptance.md) and [D062](../discovery/D062-licence-and-public-repository.md): the audit of tree and history, the rewritten public copy (0 problems), and the owner's approval. Publishing is the owner's step, from `master`. |

## Not done, and why

These are recorded where they belong:

- The API gaps T054–T058 are a deferred scope, S13.
- Jobs while InfraWerf is closed are excluded (D061).
- A paused schedule is proven only by a core test (T049).
- The code-signing and installer work for a public release is outside S12.

## Next

- Merge `feature/s12-extensibility` into `master`, when the owner says so.
- Publication then follows [PUBLISHING.md](../PUBLISHING.md).
