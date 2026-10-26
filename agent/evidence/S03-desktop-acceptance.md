# S03 — Secure desktop connections acceptance

Date: 2026-10-26<br>
ID: S03<br>
Revision: 7c2e941 + T013 working tree<br>
Result: pass

## Proof

- [T012](T012-acceptance.md) passed real OpenSSH trust and identity fixtures, including unknown/changed host rejection and the existing Windows agent. Native Settings verification preserved SSH config, trust and SQLite; no keys were copied into Atlas.
- [T014/T015](F06-F08-desktop-acceptance.md) passed actual protected SFTP session expiry/lock/revoke/disconnect clearing, stale-handle denial and child/lease cleanup.
- [T013](T013-progress.md) passed a fresh Windows Tauri UI run against an isolated Linux OpenSSH/sudo fixture: protected session, cancel, wrong password, fresh per-action success, repeat challenge and Lock. Fixture secrets were absent from persistent/UI output; map database, config and trust stayed unchanged. The pre-authentication host-fingerprint gate and spoof regression passed, with bounded independent review.

## Limits

No real server was elevated or modified. This scope proves the native client and controlled remote fixture; owner acceptance across configured LAN hosts remains part of T026. Pairing/coordinator access remains future S09.

## Rollback

Revert the T013 working-tree changes and their evidence only. Preserve the existing native database, Python reference data, SSH configuration, known-hosts and key material.
