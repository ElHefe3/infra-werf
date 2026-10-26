# InfraWerf project instructions

Start at [agent/AGENT.md](agent/AGENT.md), also referenced by [.agent.md](.agent.md). Read the active scope and feature packet before changing behavior; do not load the whole tracker by default.

Preserve the database, existing host access boundaries and approved designs. The Python app is provisional. The target is React/TypeScript/React Flow, Tauri on Windows and native Android, Rust, SQLite and system OpenSSH/local SSH agent. Fetch protected information through authenticated sessions, not pre-collection and UI hiding.

Use project-local mise tasks and exact pins established by T001. Start with latest stable Node.js Current, Rust, pnpm and compatible dependencies; never silently fall back to the old PATH Node or change unrelated global defaults.

Capture meaningful findings in `agent/discovery/`. Never include credentials or raw protected data in records. Close tickets only with evidence. The documentation task does not authorise execution of application tickets.
