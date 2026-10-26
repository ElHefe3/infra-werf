# F15 — Future compact native mobile client

<!-- atlas-feature: {"id":"F15","scope":"S09","depends_on":["F04","F02"],"deferred":true} -->

Owner decision, 2026-10-26: complete desktop 100%; mobile is future development. Preserve these requirements for explicit later activation. Desktop release does not depend on this feature. Pairing/coordinator requirements remain in F05; all phone file/transfer/terminal/service and device acceptance journeys remain in S09.

## T009 — Implement native Android client adapters

<!-- atlas-ticket: {"id":"T009","feature":"F15","status":"deferred","depends_on":["T002","T003","T007"],"evidence":null} -->

### Outcome and included work

Run the compact shared React interface in a native Android Tauri app with correct platform adapters.

### Exclusions

Direct phone SSH key storage, Android shell execution, Play Store/iOS work or final pairing claims.

### Implementation and contracts

Implement the approved separate compact phone composition and touch navigation/pan/zoom without overwriting desktop coordinates (moved intact from T006/T007 by the owner's desktop-first priority on 2026-10-25). Reuse desktop components and contracts. Implement coordinator transport interface, protected credential-store adapter and Android scoped picker/URI streaming. Do not assume filesystem paths from phone URIs. Guard mobile lifecycle and network errors; use controlled transport fixtures until real pairing passes.

**Ticket dependencies:** [T002](F01-build-environment.md), [T003](F02-domain-persistence.md), [T007](F03-react-map.md).

### Acceptance criteria

- [ ] Native Android shell renders approved UI and uses the shared operation contracts.
- [ ] Separate compact phone composition at 320/390px is readable, accessible and overflow-free with large saved inventories; search/focus and touch pan/zoom work without changing desktop positions or nested workload identity.
- [ ] File-picker/URI streams and protected credential storage are exercised through native adapters without broad all-files access or plaintext credentials.
- [ ] Disconnect/background/resume states are explicit; fixture transport cannot be mistaken for successful production authentication.

### Verification

One compact-phone journey at 320/390px, including touch navigation/pan/zoom and independent arrangement; one Android native adapter smoke test/build, including picked-file URI and lifecycle error path. Real pairing is F05 and physical acceptance T026.

### Evidence and closure

Use a concise [feature acceptance record](../evidence/README.md), map results to T009, attach its relative path in ticket metadata, then check only proven criteria. Record tested revision/platform/fixtures, failures, limits and rollback as applicable. Current proof: **not run**. Use [the checkpoint](../evidence/CHECKPOINT.md) at handoff.

### Stop conditions

Record missing native/device facilities instead of substituting browser-only proof.
