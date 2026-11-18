# Contributing to InfraWerf

Thank you for helping. InfraWerf is a desktop workspace for your own servers: every instance belongs to the person running it, and nothing leaves their PC unless they ask. Contributions keep it that way.

## Ways to help

- **Write a plugin.** Most new features belong in a plugin, in your own repository, under your own licence. Start with the [plugin author guide](docs/plugin-api/guide.md). If the API cannot express what you need, open an issue describing the gap.
- **Report a bug** in an issue, with steps to reproduce. Security problems go to [SECURITY.md](SECURITY.md), never to a public issue.
- **Change InfraWerf itself.** For anything beyond a small fix, open an issue first so we can agree on the approach before you spend time on it.

## Building

Windows 11 x64 is the release target. [BUILDING.md](BUILDING.md) has the full setup; for the desktop alone you need [mise](https://mise.jdx.dev/), Microsoft C++ Build Tools (C++ x64 workload and Windows SDK) and WebView2. Then, in the repository:

```powershell
mise trust
mise install
mise run deps
mise run check
mise run smoke:desktop
```

`mise run smoke:desktop` builds a debug executable under `target/debug/`. The Android tool chain is needed only for Android work.

## Before you open a pull request

Run what your change touches, and say in the pull request what you ran:

```powershell
mise run check
mise run exec -- cargo test --workspace --locked -- --test-threads=1
mise run exec -- node --test ui/
python -I scripts/public_repo.py check
```

Changes to the desktop's behaviour also need the native suites, which drive the real application against disposable loopback fixtures: run `mise run ssh:fixture` once, build with `mise run exec -- pnpm tauri build --no-bundle -- --locked`, then `mise run test:desktop:full` with `ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe` (the Linux container needs Podman).

## Rules that keep owners safe

- **Tests never touch real hosts.** Use the loopback fixtures in `scripts/`. A test that needs a real server is not merged.
- **No credentials, keys, addresses or names from your own lab** in code, tests, fixtures, screenshots or records. `scripts/public_repo.py check` catches the common cases; you are responsible for the rest.
- **Protected data is fetched through an authenticated session when it is shown**, not collected in advance and hidden in the interface.
- **The plugin API is a promise.** Read [what InfraWerf keeps stable](docs/COMPATIBILITY.md) before changing anything a plugin can see. Manifest changes go through `crates/atlas-core/src/plugin.rs`, regenerate the schema (`mise run exec -- cargo run -q -p atlas-core --bin infrawerf-plugin -- schema --write`), and update the reference and the guide; tests check that they agree.
- **Owner decisions are recorded.** Design choices that affect what InfraWerf may do on someone's servers are written up under `agent/discovery/`, and work is tracked as tickets with evidence under `agent/` ([agent guide](agent/AGENT.md)).

## Licence of contributions

InfraWerf is licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in InfraWerf, as defined in the Apache-2.0 licence, is dual-licensed as above, without any additional terms or conditions.
