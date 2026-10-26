# T023 acceptance — Scoped workload control

Date: 2026-10-26  
Revision: `df9e991` (the source tree was exercised immediately before this commit).<br>
Result: pass  
Feature/ticket: F11 / T023

## Environment and target

- Windows x64, native Tauri/WebView2 app; Rust 1.99.1 MSVC, Node 26.12.1 and Tauri CLI 2.12.4.
- Disposable Debian/OpenSSH workload fixture in the local Podman VM. SSH was bound to loopback. All system and user service/container changes were confined to the fixture.
- No production host was contacted. Server 1 remains corrupted, offline and powered off as reported by the owner.

## Acceptance mapping

| T023 criterion | Evidence | Result |
| --- | --- | --- |
| Start, stop and restart work for declared systemd and container adapters | Native journey completed all three actions for system systemd, user systemd, rootless Podman and system Docker. System Docker discovery, inspection, logs and control stayed on `/var/run/docker.sock` despite hostile `DOCKER_HOST`/`DOCKER_CONTEXT`. Successful systemd oneshot start/restart reported completion with observed state `Stopped`. | Pass |
| Wrong scope/identity, expired session, denied sudo and duplicate requests cannot control an unauthorized workload | Native journey denied unknown workload IDs, a session used with a second host, stale/cancelled tickets, and repeat confirmation; wrong/no-sudo credentials dispatched no action. A focused Rust test verifies an expired session cannot invoke its control executor. | Pass |
| Outcomes reflect observation, and credentials/protected output do not persist | Native proof covered running/stopped/unknown states, cancellation, Lock during an in-flight action, and an SSH response-channel loss after dispatch. Lock cleared protected results while the uncertainty notice remained; reconnect inspection confirmed the service reached `Running`. SQLite/WAL/SHM and map/event snapshots remained unchanged across protected reads; synthetic logs and credentials were absent from proof output. | Pass |

## Verification

- Native Windows journey passed: [sanitized result](../../.tools/native-workload-control-proof-ba037982-83b1-4af3-a74e-d1cc8d2caad3/result.json). It reports 28 successful assertions/results, 16 fixture control commands, `realHostsUsed: false`, `protectedOutputPersisted: false`, and no WebView page errors.
- `cargo test -p atlas-core --lib workload --locked --offline` with `CARGO_PROFILE_TEST_DEBUG=0`: **13 passed**, including the new expired-session dispatch guard, workload protocol/identity tests, one-shot semantics, cancellation, timeout and Docker socket consistency.
- `node --test ui/client.test.ts`: **4 passed**, including the workload-control IPC adapter contract.
- TypeScript `tsc --noEmit`, Vite production build, Windows Tauri `--debug --no-bundle` build and `rustfmt --check` passed. Vite retains the existing 500 kB chunk-size warning; Cargo reports existing ts-rs attribute-parser and incremental hard-link fallback warnings.
- Independent bounded review: **Pass** after repairs to Docker daemon binding, oneshot results, warning retention and stale-session reconnect state.
- The unfiltered Rust library suite was attempted but not completed: it encountered a `link.exe LNK1104` output-file lock in the default debug profile; the low-debug build then ran the suite but an unrelated health cancellation test failed and two local HTTP health tests exceeded 60 seconds. The focused T023 workload filter passed; no full-suite pass is claimed.

## Repairs and limits

Review required pinning system Docker across SSH and sudo identities, interpreting successful oneshot completion without claiming an active service, and keeping a non-sensitive uncertainty notice outside protected results. Native refinement also fixed pending UI state after an unknown-result lock and a stale-response race when Lock wins before confirmation returns. See [D035](../discovery/D035-workload-control-daemon-and-recovery.md).

This establishes the declared core adapters against synthetic controlled workloads. It does not establish compatibility with every owner server or authorize control of production workloads. Fixture state was disposable and cleaned up automatically.
