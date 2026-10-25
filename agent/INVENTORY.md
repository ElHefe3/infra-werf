# Inventory retention classification

Applies to F02/T003–T005 and contract version 1. Source requirements: [STACK.md](../STACK.md), [SCOPE.md](SCOPE.md) and the original conversation's overview / SSH / sudo boundary. The provisional `atlas.py:inventory` allowlist is a parity reference, not permission to retain arbitrary inspection output.

| Data | Treatment | Reason |
| --- | --- | --- |
| Map IDs, owner names/connection hosts, coordinates, creation times, directional relationships | Retain unchanged after validation | Owner-authored map state |
| Import source key and observation timestamp | Retain; source key stays internal | Stable identity and honest saved/stale labelling |
| Observed hostname, CPU count, total hardware memory | Retain | Selected static baseline inventory; no live utilisation is inferred |
| Service scope/name and saved state/substate | Retain, explicitly saved observations | Workload identity and selected saved status, not current health |
| Container scope/engine/name/image and saved state | Retain, explicitly saved observations | Workload identity and selected saved status |
| Service boot/restart and container restart configuration | Exclude from overview; require authenticated inspection if needed | Configuration is outside the baseline contract |
| Free-form inventory warnings | Exclude from overview | Their content can include raw command/inspection output |
| Raw report/command output, logs, process details, configuration contents, environment variables, credentials and unknown fields | Exclude from the native overview | Protected inspection must happen on demand after authentication |
| Existing application activity IDs/times/messages/server references | Preserve supported history | These are local map/import events, not remote service logs |
| Reachability/service health and live utilisation not measured by the reference | Unknown/null | Saved state is not a live check and absent data is not zero |

Known retained fields must have valid types, bounded sizes and nonfuture timezone-aware observation times. Reports are limited to 1 MiB each and 1,024 rows per workload category. Invalid retained input rejects the batch atomically. Unknown/excluded fields are discarded from the overview, not converted to persistent error messages. Newer imports preserve owner edits, IDs, positions and edges; equal/older observations are ignored by their actual timestamp instant.

Migration retains the untouched source and a complete legacy recovery snapshot before classification. Unclassified/excluded legacy fields remain recoverable there, rather than being exposed through native overview records or destroyed in the only copy. That recovery file is private local migration material, not a baseline API response or a new authorisation to collect protected information.

No live data cutover or authenticated collection is part of this feature. Do not silently add fields to `BaselineInventory`; update this classification and the shared contract deliberately when scope changes.
