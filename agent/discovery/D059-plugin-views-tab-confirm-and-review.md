# D059 — Plugin views: a workspace tab, owner-confirmed actions, an independent review

Date: 2026-11-17. Related: S12 / F23 T047; [D052](D052-plugin-platform-direction.md), [D057](D057-reviewed-read-only-pack-commands.md).

## Owner decisions

Asked on 2026-11-17, the owner chose:

1. **A tab in the server workspace.** A plugin's page appears as a workspace tab, in a separate WebView2 webview laid over the tab area: its own process and origin, no IPC. This uses Tauri's `unstable` child-webview feature. The alternative was a separate window like remote applications.
2. **InfraWerf asks the owner.** A view may ask for an action; InfraWerf shows the plugin, the server and the exact command in its own dialog, outside the view, and runs it only if the owner presses Run there. A click inside plugin content never counts as the owner's. This keeps D057: a pack command runs only when the owner starts it.
3. **A separate review session** reviews the isolation and tries to break it before the ticket closes.

## What the investigation found, and the design that follows

Reading Tauri 2.12.5 showed three facts that decide the design:

- **Pages served by the app itself are not ACL-checked.** Tauri treats its own custom protocols as local, and app commands from a local origin are allowed without a capability unless the app declares an ACL manifest. A plugin page served through a custom protocol could have called InfraWerf commands that do not check their caller, such as `map_state` or `remove_server`. So a plugin page is served from its own **loopback HTTP origin**, which Tauri treats as remote: app commands from it are refused.
- **Commands checked the window, not the webview.** A child webview in the main window has the window label `main`, and adding one makes every command that takes `WebviewWindow` fail. Every command now takes the calling `Webview` and admits only the webview labelled `main` in the window labelled `main`, and the ten commands that checked nothing now check too.
- **Capabilities are matched by window.** The `foundation` capability now names the `main` webview instead of the window, so a child webview inherits no core permissions.

Events from the core go to the main webview only, never broadcast. The page's own origin is served under a secret path that changes on every opening, with a strict content security policy; the page's Tauri internals cannot be removed (Tauri freezes them) but both of their routes are closed.

## Consequences

- `tauri` gains the `unstable` feature. With it, the desktop crate's test binaries needed the Common Controls manifest, so `build.rs` now embeds Tauri's own manifest in every target through the linker.
- The view message API is versioned separately (`v1`, in its paths), inside plugin API v2.
