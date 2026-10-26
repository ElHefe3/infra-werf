# Native Atlas development

The native foundation uses React/TypeScript/React Flow and Tauri/Rust for Windows x64 and Android ARM64. The Python application and `data/atlas.sqlite3` remain the reference; the foundation shell has no storage migration or SSH operations.

## Tool selection

Exact stable releases were resolved from mise, npm and crates.io on 2026-10-25:

| Tool | Pin |
| --- | --- |
| Node Current | 26.12.1 |
| pnpm | 12.15.0 |
| Rust/Cargo | 1.99.1, `x86_64-pc-windows-msvc` |
| Python (tracker/reference checks) | 3.15.1 |
| Java | mise `temurin-21.0.12+101.0.LTS`, runtime reports 21.0.12.1+1 |
| Tauri | Rust 2.12.5 / CLI and API 2.12.4 |
| React / TypeScript / Vite | 19.3.1 / 7.0.2 / 8.3.5 |
| React Flow | 12.12.0 |

JDK 21 LTS is the compatible Android build JDK, rather than selecting Java's newest feature release. The generated Tauri Android project pins Gradle 9.6.1, Android Gradle plugin 9.3.1 and Kotlin 2.2.10, and requires Android API 37. The existing NDK 27.1.12297006 is selected explicitly. Dependency graphs are committed in `pnpm-lock.yaml` and `Cargo.lock`; do not resolve latest versions during normal development.

## First setup on Windows

Install mise, Microsoft C++ Build Tools with the C++ x64 workload and Windows SDK, WebView2 and an Android SDK/NDK. The Microsoft installer requires a Windows administrator prompt. Android package licences must already be accepted or approved interactively; do not automate licence acceptance.

Create ignored `.mise.local.toml` with actual machine paths, for example:

```toml
[env]
ANDROID_HOME = 'C:\Users\your-user\AppData\Local\Android\Sdk'
NDK_HOME = 'C:\Users\your-user\AppData\Local\Android\Sdk\ndk\27.1.12297006'
```

Then, in this directory:

```powershell
mise trust
mise install
mise run deps
mise run doctor
mise run tracker:check
```

The Android Studio project is committed under `src-tauri/gen/android`; do not reinitialize it for ordinary builds. The CLI's initial generation installs additional Android Rust targets, but the smoke build compiles ARM64 only.

Tauri also requires Windows permission to create symbolic links for Android native libraries. On the observed PC a normal-user build compiled Rust successfully but failed at that link operation. Run the Android command from an approved administrator PowerShell when needed. `scripts/android-smoke-admin.ps1` checks for an administrator token, runs a smoke build, one successful incremental rerun and focused F01 checks; its transcript/exit marker are under ignored `.tools/logs/`. Elevating it requires the owner to handle UAC. This path produced and verified the APK; see [F01 acceptance](agent/evidence/F01-acceptance.md). Do not change Developer Mode or security policy automatically.

Gradle installed API 37.0 revision 2 and Build Tools 36.0.0 with licences already accepted on this PC. The API 37 platform lives in `platforms/android-37.0`; doctor matches its metadata rather than assuming a directory name. Actual phone execution and release signing remain untested.

## Commands and limits

```powershell
mise run dev:desktop
mise run start:desktop
mise run dev:android
mise run check
mise run test:feature -- F01
mise run test:feature -- F03
mise run test:desktop
mise run ssh:fixture
mise run test:ssh
mise run test:ssh-native
mise run test:files
mise run test:files-native
mise run smoke:desktop
mise run smoke:android
mise run build:desktop
mise run build:android
```

Smoke tasks produce debug native artifacts; release packaging/signing and actual phone execution are separate acceptance work. `test:integration` now runs the actual Python/Rust parity, rollback, lock and import journey on synthetic copies. Unmapped feature checks still fail explicitly. Doctor distinguishes required build prerequisites from deferred signing, device and controlled SSH-fixture checks.

Desktop Files uses actual SFTP through system Windows OpenSSH. Open a server workspace → Files → Open file session. Browse/stat, New folder, Rename, Move and Delete use the SSH account's permissions. Changes require an exact-path preview and confirmation; overwrite and recursive deletion are unavailable. Move pins the source while choosing a destination in the browser. A lost acknowledgement closes the session and requires inspection before a new attempt. Leaving Files, hiding Atlas or Lock clears protected results; confirmed writes block ordinary tab/dialog exit while applying. Idle/absolute limits are five/30 minutes. Lock sessions before quitting. Runtime execution is Windows-only; shared core/contracts compile for Android, whose paired journeys remain T009/T010. Content transfers, terminal and sudo remain separate work. `test:files` runs the actual SFTP lifecycle/mutation proof; `test:file-changes-native` runs both browse and confirmed-mutation journeys with a private fixture/database/profile. Run `ssh:fixture` once and `smoke:desktop` after UI/native changes.

F02 adds `mise run contracts:generate` (explicitly writes bindings), `contracts:check` (read-only comparison), `test:feature -- F02` and `check:android-core`. The Android core check uses the configured NDK's API 24 compiler without creating JNI symbolic links or rebuilding APKs. [CONTRACTS.md](agent/CONTRACTS.md), [INVENTORY.md](agent/INVENTORY.md) and [STORAGE.md](agent/STORAGE.md) describe the types, retention boundary and safe-copy migration. No live database cutover is automatic.

All tasks use `scripts/atlas.ps1` to prepend bins returned by `mise bin-paths`. This is required on the observed desktop: direct `mise exec -- node --version` was correct while child commands still found system Node 14 and skipped TypeScript's native optional package. Use `mise run exec -- <command> <args>` for one-off commands that launch children. A locked reinstall with `--force` repaired the original skipped package; it is recovery evidence, not the normal dependency command.

The Codex sandbox redirects application-data directories and restricts downloads. If a tool command fails for those reasons, use the actual command approval flow to run it in the normal user context. Do not change global tool defaults or disable the sandbox. See [D007](agent/discovery/D007-native-setup-boundaries.md).

Preserve pnpm, Cargo and Gradle caches. The Rust workspace uses root `target/`; Android uses its generated project's build directories and the user Gradle cache. Keep keystores and machine configuration out of Git. Native builds must not access the reference database.

References: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), [mise environment directives](https://mise.jdx.dev/environments/), [Vite requirements](https://vite.dev/guide/).
