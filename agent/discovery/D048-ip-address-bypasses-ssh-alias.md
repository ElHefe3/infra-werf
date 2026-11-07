# D048 — A raw IP can bypass the intended SSH account

2026-10-27. The owner's active InfraWerf entry was named `lab-server-2` but its Host field contained `192.168.20.12`. The Terminal reported authentication failure. The map name does not select an OpenSSH profile: the Host field does.

The configured alias `lab-server-2` resolves to `owner@192.168.20.12:22` with the existing identity. `lab-server-2` and the raw IP do not select that alias-specific account configuration. A server name, IP and SSH alias are distinct inputs.

Changed the existing entry through the actual Windows interface: Edit details → Host or IP address → `lab-server-2` → Save. Settings then displayed **SSH access verified** with the expected account and endpoint. Terminal connected and displayed the `owner@lab-server-2` shell prompt. No remote command was entered. Locked the session afterward and confirmed its contents cleared. Server 1 was not contacted.

The active window has two inactive candidates (`git-host`, `spare-pc`) and the manually added lab-server-2 entry. Do not confuse this with the older four-server schema-2 backup. The registered installation reports 0.1.3, but its executable hash differs from the tested release; the prior install verification remains unresolved. Do not claim the owner data migration/setup has completed.

Further known-server additions were not submitted: the UI helper detected owner input while opening Add server. Refresh the actual window before any further interaction; preserve the owner's current changes.
