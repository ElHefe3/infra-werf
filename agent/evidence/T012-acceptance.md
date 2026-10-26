# T012 existing SSH identities and host trust

Date: 2026-10-26
Revision: a6706d8 + T012 fixture/proof/documentation working tree; production SSH code unchanged.
Result: pass

## Acceptance mapping

- Actual system OpenSSH authenticated using the existing configured-file flow and a disposable Windows named-pipe agent. The agent alias has `IdentityFile none`, a specific `IdentityAgent` and `IdentitiesOnly no`; the fixture observed1 identity request,1 actual signature,successful authentication and0 remaining agent connections. Keys stayed inside the synthetic fixture/agent, never frontend/SQLite. This proves agent integration; it does not claim testing or changing the owner's Windows agent service/key store.
- Unknown/changed host keys stop before authentication; rejected auth/unavailable agent/no-auth server/unreachable endpoint are distinct. Host CA certificates, explicit HostKeyAlias and fallback known-host matching pass. Trusted keys/fingerprints appear in the native Settings flow; unresolved trust directs deliberate handling in the owner's SSH client, without auto-accept or trust reset.
- Current hostname/user/port validation, no helpers/proxies/forwarding/multiplexing, strict host checks, bounded output/deadlines/cancel, generic-command/forged-ticket denials and redacted error/result rules reuse T033/T014 production code and resolved review. Fixture configuration/trust and native SQLite hashes stay unchanged.

## Evidence

- `mise run test:ssh`:14 actual OpenSSH cases pass, including actual agent authentication. `.tools/ssh-proof-7c0e1f8b02a64682acf45fc01ef6d567/result.json`; `.tools/t012-ssh-proof.log`.
- `mise run test:ssh-native`: real Tauri/WebView2/Rust/OpenSSH Settings journeys, including agent, trust/auth/reachability/CA errors, output limits, cancellation and close guard pass with no page errors. `.tools/native-ssh-proof-269b4d7e-0186-44ea-ba51-92971e8bdc38/result.json`; `.tools/t012-native-proof.log`.
- Prior [T033](T033-acceptance.md) and [T014](F06-F08-desktop-acceptance.md) prove strict trust/argument and protected-session lifecycle boundaries; unchanged behavior was not rewritten or subjected to redundant broad regression.
- Bounded independent review found no actionable issues and confirmed the agent gap is covered by actual signing, with the owner-service limitation stated here.

## Environment and limits

Pinned Windows OpenSSH, Tauri2.12.5, Node26.12.1, Rust1.99.1 MSVC and private Python/AsyncSSH fixture. Only generated synthetic identities, private loopback host/config and a random local named pipe are involved. No owner keys, SSH config, known_hosts, OS service, sudoers, firewall or real server were modified. The generated agent shuts down with the fixture; its protocol is bounded and is not a production Atlas credential store.

Fresh sudo remains T013; T012 grants no elevation. Mobile/pairing stays future S09. Rollback removes only the added proof helper/fixtures/documentation; production transport remains the proven previous implementation.
