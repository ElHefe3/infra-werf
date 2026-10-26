# D020 — Desktop 100%; mobile is future development

Date: 2026-10-26

State: verified

Affected: all remaining release batches

## Finding

The owner explicitly instructed: “Go through all the batches,” followed by “Get desktop 100% and place mobile as a future development.” This replaces the previous Windows-plus-Android core release decision. It authorises sequential delivery of every remaining desktop core batch, without approval between ordinary implementation steps. Actual OS/data/credential permission boundaries and owner final acceptance still apply.

## Evidence

Direct owner messages in this task. The tracker moves T009 unchanged to F15/S09 and T010/T011 with F05 to S09. No ticket or future requirement is deleted. Counts are 33 tickets / 15 features / 9 scopes: 25 desktop core and 8 deferred. S02's already-completed desktop tickets/gates reuse existing actual-native evidence; no phone acceptance is inferred. Tracker validation/self-tests pass after the structural rebaseline.

## Consequence

Desktop transfers/terminal/elevation no longer depend on phone adapters or paired application sessions. Native Windows file streams, SSH authority/lifetime, confirmations and guard rules remain required. Windows installer/recovery/owner desktop acceptance gate release. Android composition, content URIs, protected phone storage, coordinator/TLS/pairing, app sessions, signed APK and actual-phone journeys remain unchecked future S09. Deferred agents/rsync remain outside the current release.

## Follow-up

Complete each desktop feature with focused proof and a local commit, then continue to the next authorised desktop batch. Preserve scope/release evidence and record external/manual blockers honestly. Activate S09 separately after desktop completion.
