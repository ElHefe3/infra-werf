# D030 — Windows ConPTY startup needs explicit handle and trust setup

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T021

## Finding

The native ConPTY child needs the pseudoconsole handle value passed directly through `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`, and `STARTF_USESTDHANDLES` set with zeroed standard handles when the GUI parent has redirected standard I/O. Otherwise OpenSSH can remain alive and emit terminal bytes without attaching to the intended input/output pipe.

OpenSSH's long-running `-E` diagnostic file can remain empty during an active interactive shell even after the remote login shell starts. It is not a reliable live authentication signal. T021 now proves fresh public-key authentication through a short fixed `true` probe, pins the authenticated session's exact host key in a temporary `known_hosts` file, and keeps that pin alive for the terminal process.

## Evidence

- Microsoft’s [ConPTY startup example](https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session#preparing-for-creation-of-the-child-process) passes the `HPCON` value itself to `UpdateProcThreadAttribute`.
- The [Microsoft Terminal ConPTY discussion](https://github.com/microsoft/terminal/discussions/15814) documents redirected parent stdio bypassing ConPTY and the `STARTF_USESTDHANDLES` correction.
- On 2026-10-26, the Windows Tauri/WebView2 proof in `scripts/verify_native_terminal.mjs` passed against a disposable loopback OpenSSH fixture: Unicode input/output, remote PTY resize, Ctrl+C, Lock cleanup, stale-input rejection after fresh authentication, and no terminal markers in the app database. The fixture SSH configuration and trust files remained unchanged.

## Consequence

Keep the ConPTY startup flags and direct handle value. Verify identity and public-key authentication before exposing an interactive shell, then bind the terminal connection to the same host key. Do not wait for the long-lived shell's OpenSSH debug log to flush. Keep shell input/output transient and close the process and key pin with the protected session.

## Follow-up

T021 records the implementation and fixture-only proof. Owner-host behavior and Android keyboard/lifecycle remain outside this acceptance; mobile remains in S09.
