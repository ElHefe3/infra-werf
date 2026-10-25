# T033 desktop SSH verification acceptance

Date: 2026-10-25
Revision: working tree based on **2720d46**; final local completion commit recorded by Git/checkpoint.
Result: pass
Ticket: **T033**, F04/S02 desktop connection verification extracted from T012/T014. No protected SSH session, paired transport, sudo, files or terminal acceptance is claimed.

## Environment and fixture

Pinned mise Node 26.12.1, pnpm 12.15.0, Rust 1.99.1 MSVC, Python 3.15.1; Windows system OpenSSH 9.5p2. Isolated `.tools/ssh-fixture-venv` with exact test requirements in `scripts/ssh-fixture-requirements.txt`: AsyncSSH 2.24.1, cryptography 50.0.2, cffi 2.1.1, pycparser 3.12, typing_extensions 4.16.0. No system services, real keys/accounts, known-host/config changes or infrastructure actions.

The fixture binds only 127.0.0.1 on ephemeral ports and generates test-only ED25519 identities. Private fixture key ACL is restricted to the current user. Native tests set an explicit private config/database/profile and never use the owner's SSH files. Fixture artifacts stay ignored under `.tools/`.

## Actual process proof

`mise run test:ssh` / `scripts/verify_ssh.py` passed **13 real OpenSSH cases**: trusted, unknown-key, changed-key, rejected-auth, absent-agent/identity, keyless-auth rejection despite spoofed success text, CA host certificate, unqualified trust fallback, explicit alias, unreachable, unsupported proxy, timeout and output limit. Final private proof: `.tools/ssh-proof-ddbb7f3a18fe44da833665088f47f875/result.json`. Config plus both trust-file digests stayed equal; unknown/changed endpoints saw zero authentication attempts and zero commands. All SSH connections closed. The fixture deliberately configures insecure loopback trust bypass and MD5 display; Atlas's CLI overrides preserve strict SHA256 verification.

## Native journey

`mise run smoke:desktop` built the Windows app. `mise run test:ssh-native` passed actual WebView2 → Rust IPC → system OpenSSH → loopback SSH:

- Settings shows a real verified response, resolved endpoint and exact host fingerprint with a clear completed-check/no-persistent-session label.
- Unknown/changed keys, rejected authentication, unreachable host and unsupported proxy show distinct states and safe guidance; no automatic trust acceptance.
- Excessive output produces a bounded error without partial payload.
- During a real slow SSH check, native close is prevented and the visible message explains active work. Cancel returns a safe error; pending native operations and server connections return to zero.
- Cross-server ticket substitution, missing server ID and a generic shell command are denied.
- Exact SQLite bytes, map/activity content, SSH config and known-host digests are unchanged by verification. Zero page errors; no private-key path or raw marker in UI.

Final native artifacts: `.tools/native-ssh-proof-465f1eab-691d-4f55-b2d0-9a2d23383008/` with `result.json` and verified/changed/cancelled screenshots, including certificate success and keyless-auth rejection after review repairs. Screenshots were visually inspected for approved desktop styling/readability. An initial rejection assertion caught outside the page lost the structured AtlasError in Playwright's wrapper; the harness now catches IPC rejection inside the page. A fixture startup deadline also exposed slow Windows account-name lookup during temporary-key ACL setup; numeric current-user SID grants and a bounded startup allowance fixed the test setup. No global ACL/service/security-policy change.

## Boundaries and repairs

Connection checks use a distinct backend-only bootstrap ticket. They do not register a fictional authenticated SSH session and cannot be used through protected inspection execution (or vice versa). Captured child stdout/stderr consumes the shared sticky output budget but only a safe DTO is returned. The actual executor retains the native admission lease through kill/wait/pipe joins, even after the caller's cancellation/deadline returns.

Strict host checking, no trust updates/DNS trust, no password/keyboard prompts, no agent/X11/port forwarding, no multiplexing, no backgrounding/local/remote commands are enforced by CLI options. Proxy/jump/known-host helpers are refused rather than run. Existing trusted local OpenSSH configuration is evaluated by OpenSSH; Atlas does not sandbox owner-defined configuration hooks. Actual jump/helper support remains T012. Configured public-key file auth is proven; a real loaded OS agent is not established/claimed by this fixture.

The first real check found HostKeyAlias's nonstandard-port behavior. Review then identified the stronger compatibility requirement: preserve an absent alias so OpenSSH retains default port lookup, unqualified fallback and certificate-principal semantics. Explicit aliases are retained. Review also caught keyless authentication being mistaken for public-key identity verification and inherited MD5 diagnostic output. Actual publickey success is now required, any none-auth success rejects the check (including spoofed accompanying text), and strict localhost checking/SHA256 diagnostics are forced. Real fixture and unit regressions cover all repairs. No trust bypass was introduced.

## Final checks and review

`mise run test:feature -- T033`: **34 Rust tests passed (32 core + 2 native), plus 4 client/model tests**, generated contract comparison, TypeScript and tracker self-tests. `mise run check` passed bindings/types/fmt/Windows compilation; `check:android-core` passed portable ARM64 compilation without APK/UAC work. Private summaries: `.tools/logs/t033-feature-final.log`, `t033-check-final.log`, `t033-android-core.log`.

`mise run test:desktop` passed the existing actual-native map save/reload/restart, keyboard, inventory, two laptop sizes, single-instance and startup-error journey on the final build: `.tools/desktop-proof-cb7d6a3e-a733-40d4-a02c-0765d79300eb/result.json`.

Independent bounded review confirmed all three P2 code findings resolved and requested adding the fallback trust file to digest checks. That evidence adjustment and final native rerun passed. No remaining review blocker. The owner's original database and real SSH trust/config were not test targets; no real host connection or owner release acceptance is claimed. Protected sessions, loaded real OS-agent proof, paired clients and operations remain their tickets.
