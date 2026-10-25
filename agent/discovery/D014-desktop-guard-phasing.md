# D014 — Separate desktop guard foundation from phone integration

Date: 2026-10-25. State: **verified**. Tickets: T008, T009, T012, T014, T032.

## Finding

The scope/ticket model deliberately prevents S03 execution before all S02 gates pass. T014 additionally depends on T010 pairing. Following that unchanged order would make the owner's requested desktop-first progress wait for compact-phone work.

## Evidence

`agent/check_tracker.py` validates prerequisite scope completion for ready/in-progress/done tickets. F06/T014 and S03 metadata retain those rules. New T032 in F04/S02 extracts only the shared desktop operation-guard foundation, depending on completed T003/T008. T014 now reuses T032 and still depends on T010. Tracker self-tests passed unchanged with 32 tickets / 27 core / 5 deferred.

## Consequence

This is a technical work split, not added product scope, a fake S02 completion or a security prerequisite bypass. T009, T014 and combined S02/S03 gates remain open. The guard itself exposes no new protected-operation IPC, SSH or network executor. Do not describe backend grants from controlled tests as successful production authentication.

## Follow-up

Reuse [OPERATIONS.md](../OPERATIONS.md) for the next desktop SSH slice. Before moving actual SSH integration ahead of phone pairing, explicitly stage the relevant ticket/gate proof while preserving real authentication, host trust, child cancellation and protected-session requirements. Do not repeatedly build unrelated phone assets or silently relax the checker.
