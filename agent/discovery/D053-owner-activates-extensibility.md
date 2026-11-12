# D053 — Owner activates extensibility (S12) and starts T037

Date: 2026-11-12. Related: S12 / F18 / T037.

The owner said in chat: "Start with the first ticket (we are only going to commit to master after the entire feature set is done)". This activates S12 and authorises T037.

## What this changes

- **Tracker:** S12 and its features F18–F25 are no longer deferred. The tracker rules activate a scope as a whole, so T038–T051 are `todo`; only T037 is started. Later tickets still follow their dependencies and stop conditions.
- **Branch policy:** all S12 work is committed on `feature/s12-extensibility`. It is merged to `master` only when the whole feature set is done. Nothing is pushed without the owner asking.
- **Dependency correction:** the draft made S12 depend on S11 and T038 depend on T035. S11 is not accepted yet (T035 awaits independent review, T036 is `todo`), which would have blocked T037 for a reason unrelated to it. S12 now depends on S06 only, and T038 converts remote applications as shipped in 0.1.5.

## What it does not change

- F18–F20 widen no security boundary. T037 only adds registries and routes existing built-in behaviour through them.
- The widening of SCOPE.md rule 6 (allowlisted operations) for plugin-declared commands is still an open owner decision, needed before F21.
- F23 and F24 each still need a security review before their tickets start.
- The working assumptions in [D052](D052-plugin-platform-direction.md) remain unconfirmed.
