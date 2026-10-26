# S05 acceptance — Observe and operate

Date: 2026-10-26  
Scope: S05  
Revision: `df9e991` for the S05 closeout; earlier feature proofs retain their per-ticket revisions.<br>
Result: pass

## Acceptance gate

| Gate | Evidence | Result |
| --- | --- | --- |
| Bounded HTTP/TCP health checks show independent host, service and freshness states with explicit failures | [T019 acceptance](T019-acceptance.md): native loopback HTTP/TCP journey, stale/failure states, timestamp retention and bounded checks. | Pass |
| Authenticated inventory refresh preserves owner data and excludes unapproved protected output | [T020 acceptance](T020-acceptance.md): native loopback SSH refresh, strict field filtering, partial/authentication failure retention and map preservation. | Pass |
| Native terminal input, resize, backpressure and lifecycle work; stale sessions cannot replay input | [T021 acceptance](T021-acceptance.md): actual Windows ConPTY journey, Unicode, resize, Ctrl+C, output cap, Lock/reconnect and stale-input rejection. | Pass |
| Workload inspection and scoped control use correct runtime/host permissions, deliberate review and truthful post-action state while logs remain protected | [T022 acceptance](T022-acceptance.md) and [T023 acceptance](T023-acceptance.md): profile-bound inspection, bounded logs, all declared control adapters, fresh sudo, unknown-result lock/reconnect and no sensitive persistence. | Pass |

## Limits and next scope

Native journeys used isolated loopback fixtures and do not claim production-host compatibility. Server 1 was not contacted. Windows packaging, integrated recovery and owner cutover remain S06 (T024–T026). Android/mobile remains future S09.
