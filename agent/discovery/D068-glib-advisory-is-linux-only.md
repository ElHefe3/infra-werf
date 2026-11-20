# D068 — The glib advisory (GHSA-wrw7-89jp-8q8g) affects a Linux-only dependency

Date: 2026-11-19. Related: [BUILDING.md](../../BUILDING.md), [SECURITY.md](../../SECURITY.md).

## Finding

GitHub's Dependabot opened alert 1 on the public repository for `glib` 0.18.5 in `Cargo.lock`. The advisory is GHSA-wrw7-89jp-8q8g: unsoundness in the `Iterator` and `DoubleEndedIterator` implementations of `glib::VariantStrIter`. It affects `glib` 0.15.0 up to but not including 0.20.0.

## What was checked

- `cargo tree -i glib --target x86_64-pc-windows-msvc` prints nothing. The crate is not compiled for the Windows release target.
- `cargo tree -i glib --target x86_64-unknown-linux-gnu` shows it comes from `atk` and `gtk` 0.18.2, which `tauri` 2.12.5 reaches through `muda` and `tao`. These are Tauri's GTK 3 bindings for Linux.
- Nothing in this repository calls `glib::VariantStrIter`.

## Why it is not fixed by upgrading

The patched `glib` 0.20 does not work with the GTK 3 bindings (`gtk` 0.18) that Tauri 2 uses on Linux. Moving to a patched `glib` needs a Tauri release that drops those bindings. The pinned Tauri version is kept until the next planned dependency update ([BUILDING.md](../../BUILDING.md)).

## Other crates in the same position

OSV-Scanner also flags `proc-macro-error`, `zvariant`, `zbus_names` and `zvariant_derive` (RUSTSEC-2024-0370 and RUSTSEC-2026-0370 to 0372). All four are "unmaintained" notices, not vulnerabilities. `cargo tree -i <crate> --target x86_64-pc-windows-msvc` prints nothing for each, so they come only from Tauri's Linux stack and are dismissed for the same reason.

## Decision

The alert is dismissed on GitHub as "vulnerable code is not used", with a comment pointing here. Linux is not a supported platform. Revisit it when Tauri moves off the GTK 3 bindings, or if Linux support is added.
