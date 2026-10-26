# D043 — Current Windows SSH agent has no loaded identities

Date: 2026-10-27<br>
State: verified<br>
Affected: T026

## Finding

The aliases for `lab-server-2`, `lab-server-3` and `lab-server-4` resolve through the current Windows OpenSSH configuration, and each configured identity-file path exists. A bounded, strict-host-checking, batch-mode `ssh alias true` check did not authenticate to any of them (exit 255). `ssh-add -l` reported no identities loaded in the current agent. No error details containing account or address values were retained.

## Evidence

One read-only remote command attempt per host used `BatchMode=yes`, `StrictHostKeyChecking=yes` and an eight-second connect timeout. The result was classified as authentication failure; the follow-up local check found configured identity files present and an empty SSH agent. This does not establish that the hosts are offline or that their keys are invalid. No password/passphrase prompt was attempted, no identity was loaded, and no SSH configuration or known-host file was changed.

## Consequence

This result describes the shell context only. It is not a device-wide conclusion about the installed Atlas app. On 2026-10-27, the installed app successfully authenticated its configured existing identity to hosts 02–04; see [D044](D044-installed-app-review-and-ssh.md). Do not load or unlock credentials based on this shell result. Continue to skip server 1, which is corrupted, disconnected and powered off.

## Follow-up

The follow-up strict read-only check was completed through the installed Atlas app on hosts 02–04; see [D044](D044-installed-app-review-and-ssh.md). Preserve the shell result as context-specific evidence and do not repeat credential prompts.
