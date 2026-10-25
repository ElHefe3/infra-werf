# D002 — Rust defaults to GNU although MSVC is installed

Date: 2026-10-25
State: verified
Affected: T001, T002, T025

## Finding

Inspected Rust/Cargo report `1.97.0`. Rustup lists `stable-x86_64-pc-windows-gnu` as active/default, plus `stable-x86_64-pc-windows-msvc` and `1.97.0-x86_64-pc-windows-msvc`. Windows SDK include directories `10.0.19041.0` and `10.0.22000.0` were observed.

## Evidence

Read-only compiler/version, toolchain-list and SDK directory inspection during planning. This does not establish C++ linker/component availability or successful Tauri compilation.

## Consequence

Select/verify the MSVC host per project, without changing unrelated global defaults. Install Android targets for that same pinned stable Rust release.

## Follow-up

T001 establishes mise selection; T002 proves Windows and Android builds early. The installed version is an observation, not the required latest pin. Sources: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), [mise Rust](https://mise.jdx.dev/lang/rust.html).
