# D051 — Owner activates remote applications and loopback forwarding

Date: 2026-11-08. Related: S11 / F17 / T035.

The owner activated S11 in chat: "Feel free to make the changes and push, there are no production dependencies, just me. Make the improvements and changes and showcase it to me." This authorises implementing T035, committing and pushing to the existing `origin` remote.

Decision: the SSH boundary that excluded forwarding (F04 T033) is widened only for owner-started local forwards of the form `127.0.0.1:<ephemeral> → localhost:<remote-port>` on the selected server. Remote (`-R`) and dynamic (`-D`) forwarding, LAN/0.0.0.0 listeners, agent/X11 forwarding and forwards defined in the owner's OpenSSH configuration remain excluded; an alias whose configuration defines forwards is refused rather than silently cleared or applied. Verification, strict host-key checking and BatchMode stay unchanged. Nothing is installed on servers.
