# Build InfraWerf from source

Use this guide to run a development build, produce the installer yourself, or start contributing. For how to use the app, see the [user guide](user-guide.md). For the exact pinned versions and Android notes, see [BUILDING.md](../BUILDING.md).

## What you need

| Tool | Version pinned in `mise.toml` | Why |
| --- | --- | --- |
| [mise](https://mise.jdx.dev/) | 2026.9.1 or newer | Installs and runs every other tool in the pinned versions. |
| Node.js (Current) | 26.12.1 | Builds the React interface. |
| pnpm | 12.15.0 | Installs JavaScript dependencies from the committed lockfile. |
| Rust (MSVC) | 1.99.1, `x86_64-pc-windows-msvc` | Builds the Tauri shell and the core. |
| Python | 3.15.1 | Optional: tracker checks and the Python reference. |
| Microsoft C++ Build Tools | C++ x64 workload and Windows SDK | Required by Rust on Windows. |
| WebView2 Runtime | current | Renders the interface. |

Android tools are only needed for Android work, which is not part of the release.

You do not need to install Node, pnpm or Rust yourself. mise installs the pinned versions into the project, and nothing global changes.

## First build

From the repository root, in PowerShell:

```powershell
mise trust
mise install
mise run deps
mise run doctor
mise run check
mise run smoke:desktop
```

- `mise trust` allows mise to read the project's `mise.toml`.
- `mise install` installs the pinned tools.
- `mise run deps` installs JavaScript dependencies from `pnpm-lock.yaml`. Rerun it only when the lockfile changes.
- `mise run doctor` reports the tool versions and prerequisites it finds. Fix anything it flags before going further.
- `mise run check` type-checks the interface and the Rust code without changing files.
- `mise run smoke:desktop` builds a **debug** executable under `target\debug\`. It is for development only; do not distribute it.

## Run the app in development

```powershell
mise run dev:desktop
```

This starts a development window that reloads when you edit the interface.

To open the app as an installed user would, without a dev server:

```powershell
mise run start:desktop
```

## Build the installer

```powershell
mise run build:desktop
```

This produces an optimised NSIS installer at:

```text
target\release\bundle\nsis\InfraWerf_<version>_x64-setup.exe
```

Check the file name against the output, since the version is taken from `src-tauri\tauri.conf.json`. Use this installer for distribution, not the `smoke:desktop` output. Release steps and checks are in [agent/RELEASE.md](../agent/RELEASE.md).

## Running commands

Some tools launch child programs. Run one-off commands through the project's wrapper so they find the pinned tools:

```powershell
mise run exec -- cargo test --workspace --locked -- --test-threads=1
mise run exec -- node --test ui/
```

Running `node` or `cargo` directly from a shell may pick up an older system install.

## Troubleshooting the build

| Symptom | Likely cause and fix |
| --- | --- |
| `mise trust` asks you to confirm | Expected the first time. Review `mise.toml` and confirm. |
| `link.exe not found` or C++ errors | Install the Microsoft C++ Build Tools with the **C++ x64 workload** and a Windows SDK, then open a new terminal. |
| WebView2 errors at launch | Install the [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/). |
| Wrong Node or Rust version used | Run through `mise run exec -- …`, and check `mise run doctor`. |
| `pnpm install` fails on a changed lockfile | Do not upgrade packages mid-feature. Use the committed lockfile, or ask in an issue before changing it. |
| Android build fails on symbolic links | Windows needs permission to create symbolic links for Android. See [BUILDING.md](../BUILDING.md#first-setup-on-windows). Android is not required for desktop work. |

More symptoms and fixes for running the app are in [troubleshooting](troubleshooting.md).

## Next

- [Testing](testing.md): what to run before a pull request.
- [Contributing](../CONTRIBUTING.md): the rules for changes, especially those touching servers.
