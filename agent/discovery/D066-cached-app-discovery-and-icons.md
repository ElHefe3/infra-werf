# D066 — Discovered apps are cached on this PC and carry their own icons

Date: 2026-11-18. Related: S14 / F27 T064, F17 T035, [D051](D051-owner-activates-remote-applications.md).

## Owner request

On 2026-11-18 the owner asked for app discovery to be cached, so that already known apps can be chosen while discovery runs again. They also asked for the Apps tab to be a tile grid with app icons filled in automatically.

## What was decided

- **Cache.** The last discovery result for each server is kept in the WebView's local storage, like remote-app shortcuts: port, bind address, address-specific destination, HTTP status, HTTPS-only flag, page title, process name and icon. It holds nothing beyond what discovery already shows. It is validated field by field on load, and a corrupt entry is ignored.
- **The cache grants nothing.** Opening an app still verifies SSH first. The core authorises a non-loopback destination only from this run's in-memory discovery, so a cached address-specific app stays disabled until a fresh discovery confirms it.
- **Icons come from the app itself.** The fixed discovery command reads the app's `<link rel=icon>` path (same origin only) or `/favicon.ico`. Each icon is at most 6 KiB and the run's icons total about 40 KB, because a discovery reply is limited to 64 KiB. The Rust parser decodes the icon, types it by its magic bytes (PNG, ICO, GIF, JPEG or WebP), re-encodes it, and drops anything else, including SVG and HTML.
- **Image policy.** The main window's `img-src` now allows `data:` so these icons can be drawn. Scripts and other sources are unchanged; plugin views already allowed `data:` images.

## Update the same day

- Opening a cached app waited for discovery to finish, because discovery held the live-contact lock for its whole remote run. It now holds the lock only while its session opens, as file sessions do; a target change during the listing still discards the result.
- Discovery also reads the page's `generator` meta tag as a version hint. App details now sit behind an ⓘ button and the PIN ([D067](D067-owner-pin.md)).

## Still open

- Larger favicons (multi-size `.ico` files are often over 6 KiB) fall back to a letter tile. A bundled icon set or a larger, separate icon request could cover them later.
