# Atlas project instructions

Read STACK.md, WORKFLOW.md and ROADMAP.md before implementation. Preserve the database and approved desktop/mobile designs.

The original stack is recovered and recorded in STACK.md: React + TypeScript + React Flow, Tauri, Rust, SQLite and system OpenSSH/local SSH agent; SFTP first, optional rsync later. The Python web implementation is provisional. Migrate it to the recorded stack while preserving data and verifying parity. Do not silently substitute frameworks. Protected information must be fetched on demand through authenticated sessions rather than pre-collected and merely hidden in the UI.

Keep confirmed scope, stack and progress in repository documents so work can continue across desktop and Android without relying on complete chat-history retrieval.
