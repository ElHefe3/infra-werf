# InfraWerf presentation script

**Length:** about 12 minutes plus questions. **Audience:** people who run their own servers or homelabs, and developers who may want to extend it.
**Version shown:** 0.1.13 (`package.json`, `src-tauri/tauri.conf.json`).

Stage directions are in *italics*. Spoken lines are plain text. Each slide ends with a one-line takeaway you can say aloud if time is short.

---

## 1. Title (0:30)

*Slide: "InfraWerf — Your homelab, connected."*

> InfraWerf is a desktop workspace for the servers you run yourself. It gives you a map of your machines and, for each one, its inventory, health, files, terminal, services and containers. It uses your own OpenSSH access. There is no account, no cloud service and no telemetry.

**Takeaway:** your machines, your access, your data.

---

## 2. The problem (1:00)

*Slide: a typical homelab: a few boxes, a terminal per box, a spreadsheet of what runs where.*

> Most homelabs grow one machine at a time. Each server has its own SSH session, its own file copies, its own `systemctl` and `docker ps`. Knowing what depends on what lives in your head. Dashboards tend to need an account, a cloud service or an agent you have to trust with everything.

> I wanted one local program that shows the whole estate, and that uses the access I already have rather than a new one.

**Takeaway:** one place to see and act on the estate, without giving anyone else your servers.

---

## 3. What it does (1:30)

*Slide: four panels — Map, Server workspace, Files and transfers, Terminal.*

> The **map** shows your machines and how they relate. You can draw directed links, rename entries, and arrange them.

> Each **server workspace** holds the saved inventory and search, the current health, and the workloads on that host: systemd services, Podman and Docker containers, and their logs.

> **Files** is a protected SFTP browser with uploads, downloads and confirmed changes. **Terminal** is an interactive shell.

> Operations that change a machine, like restarting a service or editing a file with elevated rights, need a separate authenticated session, and the app asks again for each action.

**Takeaway:** read broadly, change deliberately.

---

## 4. Trust model (2:00)

*Slide: "What it never does".*

> This is the part I'd like you to remember.

> - It reads your existing OpenSSH setup and your public-key identities or agent. It never copies a private key.
> - It requires strict host trust. Unknown or changed fingerprints must be verified through your own SSH workflow. InfraWerf does not edit your known-hosts file.
> - Saved observations are labelled with when they were taken. A server that was healthy yesterday is shown as stale, not as currently healthy, and never as "offline" just because it has not reported.
> - Its data lives in a local SQLite database. It has no passwords and no raw configuration.

**Takeaway:** it does not ask you to trust it more than you already trust OpenSSH.

---

## 5. How it is built (1:30)

*Slide: stack diagram — React + TypeScript + React Flow on top; Tauri shell; Rust core; SQLite; system OpenSSH.*

> The interface is React, TypeScript and React Flow. The shell is Tauri, which runs on Windows. Almost all the logic, including persistence, SSH and the protected operations, is in Rust. The data is SQLite, owned by the native app. SSH uses the system OpenSSH on the PC, not a bundled library.

> Android is planned, but it is future work and is not part of this release.

> The earlier Python prototype is kept as a reference and fallback. It is not the product.

**Takeaway:** a small trusted core in Rust, a thin interface on top, and the system's own SSH.

---

## 6. How it was made (1:30)

*Slide: the tracker — scopes, features, tickets, and the evidence folder.*

> This project was built as a sequence of scopes, each with features and tickets. A ticket is closed only when there is evidence on the real Windows machine. Source review or a screenshot of a mock is not enough.

> Findings and decisions are recorded in `agent/discovery/`, and acceptance records in `agent/evidence/`. Secrets and raw protected output are never written there.

> That discipline is why the desktop release has an owner acceptance record, not only passing unit tests.

**Takeaway:** the proof is in the repository, not in the claims.

---

## 7. Testing against real SSH (1:00)

*Slide: loopback fixtures, `mise run test:ssh`.*

> Protected operations are tested against real OpenSSH servers on loopback, with throwaway keys and data. The native Settings flow is tested in the real Tauri WebView2 window with synthetic data. No real host, trust file or password is touched by the tests.

**Takeaway:** the risky paths run for real, against fixtures.

---

## 8. Plugins (1:30)

*Slide: plugin manifest snippet from `examples/`.*

> InfraWerf can be extended with plugins. A plugin is a folder with a manifest, which declares what it needs and what it can show. It can add data, panels, views, and small jobs, within permissions that you grant.

> There are working examples in `examples/`, starting with backup overview. The author guide builds a plugin from an empty folder. The compatibility page says what stays stable between versions.

**Takeaway:** the core stays small; extensions say what they need.

---

## 9. Status (1:00)

*Slide: status summary.*

> The Windows desktop is the release target and the owner has accepted it as the primary app. The current release is 0.1.13. The map, inventory, SSH verification, Files, Transfers, Terminal, health, workload inspection and scoped service control are all in place.

> Still ahead: a phone interface over LAN or VPN, authenticated pairing, and Android packaging. Rsync and discovery agents are deferred.

**Takeaway:** the desktop is done and in daily use; mobile is next, not finished.

---

## 10. Limits, honestly stated (0:45)

*Slide: "Not yet / By design".*

> - Jump hosts, proxies and known-host helper configurations are not supported for direct verification. Those servers show as unavailable.
> - Saved data is a snapshot. It does not prove a server is reachable right now.
> - There is no cloud sync, by design.

**Takeaway:** what it does not do is as important as what it does.

---

## 11. Try it / get involved (0:45)

*Slide: install and links.*

> Build the installer with `mise run build:desktop`, or open the installed app from the Start menu. For developers, start with `CONTRIBUTING.md`, the plugin author guide, and the examples. Security problems should be reported privately, as `SECURITY.md` describes.

> InfraWerf is dual-licensed under MIT or Apache-2.0.

**Takeaway:** build it, extend it, or report what breaks.

---

## 12. Close (0:20)

> InfraWerf is for the person who runs the servers. It shows what is there, tells you how fresh that knowledge is, and lets you act with access you already had. Thank you. I'm happy to take questions.

---

## Likely questions and short answers

- **Does it send anything anywhere?** No account, cloud or telemetry. Connections go only to the servers you configure, over your OpenSSH setup.
- **Can it read my private keys?** It uses the public-key identities and agent that your OpenSSH already has. It does not copy private keys.
- **Why not a web dashboard?** A local desktop app keeps credentials and trust decisions on your machine, and avoids running a service on the network.
- **Does it work on Linux or macOS?** The release target is Windows 11 x64. The stack is cross-platform in principle, but only the Windows path is accepted.
- **Is my old Python data safe?** The Python prototype's database is kept as a reference and is not modified by the native app.

## Pre-talk checklist

- Run `mise run start:desktop` and confirm the map loads with your own data (not the synthetic fixtures).
- Have the Settings → Verify SSH access screen ready on a server you know is reachable.
- Decide whether to show the live Terminal; if you do, use a non-production host.
- Do not show real hostnames, addresses or key material on screen if the audience is outside your network.
