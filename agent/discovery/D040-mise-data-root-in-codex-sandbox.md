# D040 — Pin mise's data/cache roots inside the Codex sandbox

Date: 2026-10-27<br>
State: verified<br>
Affected: T025

## Finding

In the Codex process, `LOCALAPPDATA` points to a sandbox package directory, while the already installed project tools live under `C:\Users\owner\AppData\Local\mise`. Without an explicit data root, mise reported pinned Node, pnpm, Python and Rust as missing. Its first automatic Rust repair attempt was blocked from writing the user-profile installation directory. mise also could not write its default cache below the sandbox `LOCALAPPDATA` temp path.

## Resolution

For the project task run, set `MISE_DATA_DIR` to the existing user mise installation and `MISE_CACHE_DIR` to the ignored workspace `.tools/mise-cache`. Set `MISE_AUTO_INSTALL=0` and `MISE_OFFLINE=1` to prevent implicit installs, and set `RUSTUP_TOOLCHAIN`/`RUSTC` to the already installed Rust 1.99.1 MSVC toolchain. The Node 26.12.1 and pnpm 12.15.0 project pins were then resolved by mise; no tool was installed or upgraded.

The Rust junction exists under mise but its current backend metadata reports no executable, so mise warns that Rust is missing. The pinned rustup toolchain remained usable through the explicit environment above. `mise run doctor` passed all Windows desktop requirements but reported the known Android symbolic-link permission failure (EPERM), which is outside T025.

## Consequence

When a Codex shell redirects `LOCALAPPDATA`, inspect `mise doctor` and `mise dir` before concluding that a pinned runtime is absent. Keep temporary cache overrides task-local; do not overwrite global mise configuration or install copies into user paths from the sandbox.
