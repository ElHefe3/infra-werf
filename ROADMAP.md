# Atlas roadmap

The current roadmap is in [agent/AGENT.md](agent/AGENT.md). Product/release boundaries are in [agent/SCOPE.md](agent/SCOPE.md). Ticket status is canonical only in the feature packets; do not maintain a second status table here.

## Historical increment mapping

| Previous label | Current location | Meaning |
| --- | --- | --- |
| A01 | [IMPLEMENTATION.md](IMPLEMENTATION.md) | Completed historical Python/SQLite/browser reference; not completion of the native-stack tickets |
| M01 | S01–S02 | Mise/toolchains, Rust data parity, React Flow and Windows/native Android shells |
| A02 | S03 and S06 | PC-first encrypted pairing/sessions and actual phone acceptance; always-on deployment is deferred to S07 |
| A03 | S03 / F06 | Host trust, SSH and deliberate elevation |
| A04 | S04 | SFTP file manager and transfers |
| A05 | S07 | Deferred restricted discovery agents and always-on operation |
| A06 | S05 | Monitoring, terminal and scoped workload operations |

Current plan: 28 core tickets across 12 features/6 scopes, then 5 deferred tickets across 2 features/2 scopes. T032/T033 separate desktop guard/connection verification from later paired integration; product scope is unchanged. Core targets full controls on native Windows and Android over LAN/existing VPN. Latest stable tools are pinned through mise. S01 foundations and T006–T008 desktop map/lifecycle are complete; current status is in the agent guide/checkpoint.
