# D004 — The existing application is a parity reference

Date: 2026-10-25
State: verified
Affected: T003, T004, T005, T006, T007, T024, T026

## Finding

A01 delivered Python HTTP/SQLite and vanilla-JavaScript browser views. Its historical evidence reports four hosts, 540 saved workload records, ten focused tests and browser checks. These are observations, not live health/native proof. Counts can change through use; inspect current data instead of hardcoding them.

## Evidence

[IMPLEMENTATION.md](../../IMPLEMENTATION.md), [STACK.md](../../STACK.md), the existing schema/API and tests/static assets. The full recovered transcript is in the parent project folder. The owner subsequently selected native Android, Windows-first and full core phone controls.

## Consequence

Preserve original data and IDs/layout/relationships during Rust migration. Classify inventory for approved baseline retention. Do not port simulated authentication or count the existing app as completed React/Tauri/Rust work.

## Follow-up

F02 uses synthetic fixtures and a safe database copy for parity; F03/F04 preserve appearance while replacing mechanics; F12 verifies rollback and cutover before retiring the reference.
