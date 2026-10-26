# D017 — Explicit desktop-first feature entry

Date: 2026-10-26

State: verified

Affected: T009–T015, T032–T033

## Finding

The owner prioritised a usable Windows desktop while native-phone composition and pairing remain open. Requiring every phone ticket in a prerequisite scope before protected desktop work would prevent that priority. Repeatedly splitting off foundation-only tickets would also add tracker and review overhead without delivering the Files journey.

## Evidence

The owner requested “desktop working first.” T008, T032 and T033 already have native evidence. `agent/check_tracker.py --self-test` now validates an explicit per-ticket `scope_entry` mapping. Four additional rejection cases and a positive phased-entry case verify that entry IDs must belong to the declared prerequisite scope, be actual ticket ancestors and be complete. The existing full-scope default remains enforced.

## Consequence

T014 can enter over completed desktop prerequisites, and T015 can follow T014 without depending on sudo. The scheduling exception grants no operation authority and does not change scope completion, phone acceptance, pairing, revocation or elevation requirements. T009/T010 explicitly retain compact-phone Files/navigation/clearing proof; T013 retains fresh per-action sudo. Ticket/feature/scope counts remain 33/14/8.

## Follow-up

Use the explicit mapping only for an already-authorised phase. Keep scope and release gates open until all original platform journeys have evidence. Reuse the shared session/contracts/fixture instead of implementing an independent phone security path.
