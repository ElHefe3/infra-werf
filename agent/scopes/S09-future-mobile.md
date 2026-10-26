# S09 — Future mobile development

<!-- atlas-scope: {"id":"S09","features":["F15","F05"],"depends_on":["S06"],"window":null,"deferred":true} -->

The owner explicitly deferred mobile on 2026-10-26. These requirements are preserved, not accepted or removed. Activate separately after the desktop release.

## Feature batches

- [F15](../features/F15-future-mobile.md): T009 native Android compact composition, touch map, scoped content URI streams, protected storage and lifecycle.
- [F05](../features/F05-device-pairing.md): T010/T011 encrypted desktop-approved pairing, pinned TLS identity, revocable device credentials, expiry/reconnect.

## Acceptance gate

- [ ] Native 320/390px composition, independent map arrangement, touch/search and protected-view clearing pass on the phone.
- [ ] TLS pinning, single-use pairing, desktop approval, revoke and 15-minute idle/eight-hour absolute app-session policy pass real coordinator tests.
- [ ] Android scoped picker/content URI and OS-protected credential storage pass native positive/negative cases without broad storage grants.
- [ ] Phone file browse/edit/upload/download, interruption recovery, terminal, monitoring and service controls share desktop contracts and pass actual-device journeys.
- [ ] Signed ARM64 APK, controlled install/update, background/resume, owner phone acceptance and rollback have actual evidence.

No desktop feature or release gate is blocked by this future scope. No Android signing material, firewall change, public listener or deployment is created by deferral.
