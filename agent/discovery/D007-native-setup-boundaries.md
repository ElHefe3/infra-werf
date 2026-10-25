# D007 — Native setup requires the correct process context

Date: 2026-10-25. State: verified; PATH/JDK/UAC setup resolved. Affected: F01/T001–T002.

## Finding and evidence

- The configured Android SDK exists at `ANDROID_HOME`. The sandbox redirects `LOCALAPPDATA` and mise's install/cache home; probing the redirected location incorrectly suggests the SDK/tools are absent. A sandboxed mise check attempted fresh downloads and failed DNS/cache permissions after normal-user installation had succeeded. Retry the required command through the actual escalation mechanism; do not change approval policy or infer missing tools from redirected paths.
- The inherited global `JAVA_HOME` contains a trailing quote and points to a missing JDK. Atlas selects its compatible JDK 21 through mise and records SDK/NDK paths in ignored `.mise.local.toml`. Global defaults are not repaired as part of Atlas.
- `mise exec -- node --version` selects 26.12.1, while its child PATH places the system Node 14 and Python 3.12 before project tools. `where node`, the environment doctor and the failed TypeScript invocation demonstrated this. An explicit `env._.path` attempt did not repair ordering and was removed. Tasks now resolve `mise bin-paths` and prepend those directories within `scripts/atlas.ps1`; verify effective versions in child commands, not just `mise which`.
- The first Microsoft C++ Build Tools installation exited 1602. Its decompression log reports `User may have declined UAC prompt` / failure to start the elevated process. That is an administrator-prompt boundary, not proof that the user intentionally cancelled. The owner confirmed access to the PC and requested a relaunch; winget subsequently reported success and doctor located MSVC 14.44.35207 and its x64 linker.
- The initial pnpm installation skipped TypeScript 7's Windows native optional dependency (the package requires Node >=16.20). A normal frozen reinstall retained that skipped state after PATH repair. One forced frozen reinstall reused cached downloads, restored the native package and allowed `pnpm check` to pass under Node 26. This is a setup recovery, not a reason to force every install.
- The generated Tauri 2.12.4 Android project requires API 37, Gradle 9.6.1 and Android Gradle plugin 9.3.1. Existing SDK APIs 35/36 do not prove build readiness. Doctor now derives the required API from the actual generated build file and reports the missing platform.

## Consequence

Keep tool installation, native compilation, device execution and release acceptance as separate evidence. Do not retry installers blindly or claim native feasibility from installed directories. Use the project wrapper for one-off child-process commands: `mise run exec -- <command> <args>`.

## Follow-up

T002 must prove Windows launch and Android compilation, including any API 37 package/licence requirement. Preserve caches; do not repeat tool installs. Release signing and physical-phone acceptance remain separate tickets.
