# Testing

The tests run against disposable loopback fixtures, not real servers. Some of them drive the built Windows application (WebView2 and Rust IPC). This page lists the tests, what each one checks, and what to run before a change.

## Before any change

```powershell
mise run check
mise run exec -- cargo test --workspace --locked -- --test-threads=1
mise run exec -- node --test ui/
mise run tracker:check
```

- `check` type-checks the interface and Rust code without rewriting files.
- The Cargo tests run the Rust core, including storage and plugin validation.
- `node --test ui/` runs the interface's model tests.
- `tracker:check` validates the project tracker in `agent/`. It matters only if you touch that tracker.

## Feature checks

```powershell
mise run test:feature -- F03
```

Runs the smallest meaningful set of checks for one feature. The mapping of feature IDs to checks is in [agent/AGENT.md](../agent/AGENT.md).

## SSH fixtures

These run real OpenSSH against loopback servers that use throwaway keys and data.

```powershell
mise run ssh:fixture        # prepare the pinned test runtime (run once)
mise run test:ssh           # strict trust, authentication and error cases
mise run test:ssh-native    # the Settings flow in the built app (build first)
```

`ssh:fixture` creates keys and configuration under the ignored `.tools\` folder only. It does not change your SSH configuration or known-hosts file.

## Native desktop suites

These drive the built application. Build it first:

```powershell
mise run exec -- pnpm tauri build --no-bundle -- --locked
```

Then, for the complete sequence:

```powershell
$env:ATLAS_DESKTOP_BINARY = "target\release\atlas-native.exe"
mise run test:desktop:full
```

Or run a single suite:

| Task | What it proves |
| --- | --- |
| `test:desktop` | Save, restart and error paths through the real IPC on synthetic data. |
| `test:health-native` | Health states in the real window: healthy, stale, failure. |
| `test:inventory-native` | Approved inventory refresh over disposable SSH. |
| `test:workloads-native` | Workload inspection and bounded logs on fixture SSH. |
| `test:files` | SFTP session lifetime, handle revocation, timeouts and cleanup. |
| `test:files-native` | Browse, stat and lock journey in the app. |
| `test:file-changes-native` | Confirmed create, rename, move and delete, plus cancellation, in a fixture-only area. |
| `test:elevation` | Fresh `sudo` and host-pinned operations in a disposable Linux container (Podman required). |
| `test:elevation-native` | Wrong password, fresh authentication, cancel and clearing in the app. |
| `test:monitoring-native` | Monitoring failure, recovery and server lifecycle on loopback data. |
| `test:apps-native` | App tunnels: isolated window, HTTP and WebSocket, teardown, refusals. |

The `demo:apps` task opens a visible demonstration on a throwaway workspace. Close your own InfraWerf first.

## Plugin checks

Plugin behaviour is tested through the same native suites (`verify_native_plugin_*` scripts), and the manifest schema is tested to match the Rust types. If you change a manifest field, regenerate the schema:

```powershell
mise run exec -- cargo run -q -p atlas-core --bin infrawerf-plugin -- schema --write
```

Then update the plugin reference and the author guide. Tests check that they agree.

## Integration

```powershell
mise run test:integration
```

Runs the storage journeys (Python–Rust parity, rollback, writer locks and import) on private synthetic copies.

## Privacy check

```powershell
python -I scripts/public_repo.py check
```

Scans for credentials, keys and personal details before you publish. Run it before a pull request. It catches common cases; you are responsible for the rest.

## Rules for tests

- Tests never touch real hosts, real trust files or real data. A test that needs a real server does not get merged.
- Do not commit test keys, databases or logs. The `.tools\` folder is ignored for this reason.
- Test what the behaviour *means* to the owner, not how the code is built.

## What counts as proof

- A passing unit test shows the logic works.
- A native suite shows the built application works on Windows.
- A screenshot or mock does **not** show that a feature works on a real PC.

Pull requests that change desktop behaviour should say which native suites were run.

## Android

Android checks (`smoke:android`, `check:android-core`, `build:android`) need the Android toolchain. They are not required for desktop work. Phone testing is future work.
