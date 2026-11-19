# D063 — The Claude desktop shell is also a packaged (redirected) context

Date: 2026-11-18. Related: D050, [0.1.4 install record](../evidence/RELEASE-0.1.4-install.md), T035. Corrects the 2026-11-08 D050 update.

The 0.1.4 and 0.1.5 installs on 2026-11-08 ran from the Claude desktop app's shell, which is itself a packaged app (`Packages\Claude_pzs8sxrjxfjjc`). Windows redirected the program folder, its `HKCU` uninstall registration and the Tauri NSIS cache into `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Local\…`. From inside that shell the redirected copy appears at `%LOCALAPPDATA%\InfraWerf` and hides the real folder, so the install looked like the ordinary location. It was not: D050 was not resolved by those installs.

Evidence (2026-11-18): the owner installed 0.1.6 from Explorer and launched it (parent `explorer.exe`). Inside the Claude shell, `%LOCALAPPDATA%\InfraWerf\atlas-native.exe` still reports ProductVersion 0.1.5 (8 Nov) and the registration says 0.1.5. Through the admin share (`\\localhost\C$\Users\<user>\AppData\Local\InfraWerf\atlas-native.exe`) the real file reports **0.1.6**, last written 2026-11-18 17:07:42. The workspace store in `%APPDATA%\dev.atlas.workspace` is not shadowed, because no redirected copy of it was ever written; it is shared by every copy.

Consequences and rules:

- Do not install or start InfraWerf from an agent shell (Codex or Claude desktop). Ask the owner to run the installer from Explorer; verify through `\\localhost\C$\…` and `FileVersionInfo`, not the shell's `%LOCALAPPDATA%` or `HKCU`.
- `mise run start:desktop` from an agent shell launches the stale redirected copy (currently 0.1.5 under Claude, 0.1.3 under Codex). Native fixture journeys are unaffected: they use the build-tree binary and their own temporary store.
- Stale redirected copies remain under both packages; removing them is an owner decision and only changes what agent shells see.
