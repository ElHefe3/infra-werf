# D054 — Eight native suites fail at `add_server` since alias discovery

Date: 2026-11-12. Related: T034 / F16 (cause), T037 / F18 (where it was found).

## Observation

While verifying T037, eight of the twelve native suites failed: ssh, inventory, files, file-changes, terminal, elevation, workloads and workload-control. Each fails at its first `add_server` call. The terminal suite reports the error as `add_server failed` with code `conflict`.

The same eight fail at the same lines against the installed 0.1.5 release (`c0f8299`), so the failure predates the S12 work. Desktop, health, monitoring and apps pass on both.

## Cause

Not confirmed by a targeted run; inferred from the code and the error. Commit `dc66c0f` (WIP live monitoring, T034) added background discovery that records the primary aliases of the configured OpenSSH file as candidate servers shortly after start-up. The eight suites give the app a fixture OpenSSH config and then add servers with those alias names as hosts. The host column is unique, so the add conflicts. The monitoring and apps suites were written after that commit; they call `monitoring_discover` and use the discovered servers instead.

## Why it matters

- The acceptance wording "all existing native suites pass" cannot be met by any ticket until this is repaired.
- For an owner, adding a server whose alias is already a discovered candidate gives a bare conflict error. That is a T034 behaviour question, not only a test problem.

## Options

1. Repair the suites: use the discovered server for a fixture alias (as the monitoring and apps suites do) instead of adding it.
2. Change the product: `add_server` for a host that exists only as a discovered candidate adopts and activates it. The suites then pass unchanged.
3. Both.

No suite or product behaviour was changed under T037. The choice belongs with T034.
