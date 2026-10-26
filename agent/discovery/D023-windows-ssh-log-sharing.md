# D023 — Windows SSH log sharing and the pre-authentication gate

Date: 2026-10-26 · T013. State: **verified blocker**.

## Finding

T013's command channel must not release its random remote-input gate until the local OpenSSH client proves public-key authentication and the pinned host fingerprint. Windows OpenSSH opens its `-E` diagnostics file without sharing it for a new reader. Attempting to open the active log from Rust returned Win32 sharing violation 32, so authentication evidence was unavailable and T013 returned `IoFailure` before the gate.

## Evidence

The disposable loopback Linux sudo fixture in `mise run test:elevation` reproduced the wrong-password acceptance case returning `IoFailure` instead of `Unauthenticated`. A fixed-stage diagnostic reported only OS code 32; no host output or credentials were logged. A pre-created regular log file with a live Rust reader did not make the acceptance case pass. The original live-log implementation was restored afterward.

An alternative that removed the pre-authentication gate was rejected by automatic review because it weakened the protected elevation boundary. No part of that rejected patch was applied. Do not retry that approach.

## Consequence

T013 stays blocked and its acceptance criteria remain unchecked. Do not send the remote gate or sudo password until authenticated SSH evidence is available through a separate, verified channel. Do not infer success from a completed command or from the existing SFTP session.

## Follow-up

Find a Windows OpenSSH log-capture mechanism that permits live, isolated reads while preserving the current pre-authentication gate, or another independently reviewed transport with the same guarantee. Then rerun the focused synthetic sudo fixture and obtain bounded security review. T019 may proceed through its explicit T014 scope entry; T020 and the terminal/service tickets remain blocked on T013.
