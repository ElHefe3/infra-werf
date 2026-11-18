# Native suite flakes (2026-11-17)

After T046, four full native runs each failed on a different suite unrelated to the change being tested. This note gives the cause and fix for each failure seen. Branch `claude/native-flakes-6874c4`, cut from `feature/s12-extensibility` at `fadbc3c`.

## Causes and fixes

| Failure seen | Cause | Kind | Fix |
| --- | --- | --- | --- |
| `files`: after leaving Files, `stats().trusted.active` never reached 0 (15 s poll; the failure screenshot shows the Services tab) | The SSH fixture published `stats.json` with `os.replace`. On Windows that fails with `PermissionError` while another process has the file open, and the suite polls that file. Raised inside asyncssh's `connection_lost`, the error was swallowed, so the file kept `active: 1` until some later write. Nothing was logged. | Fixture | `scripts/fixture_stats.py` retries the replace for up to 5 s and logs to stderr each time it had to retry. Used by `ssh_fixture.py`, `apps_fixture.py` and `monitoring_fixture.py`, which all had the same pattern. |
| `files`: deep-folder loop stayed on the previous path; `disconnect` folder left the list visible | The same write is in the fixture's `scandir`. When it raised, asyncssh turned the `PermissionError` into an SFTP "permission denied" reply. The failure screenshot shows the app's `denied` alert ("Your SSH account cannot access this target") at `/home/fixture/deep/next`. The app handled it correctly; the fixture caused it. | Fixture | Same fix. |
| `files`, `file-changes`: overflow alert did not contain "safety limit" (screenshot: only "Session closed or expired") | **App bug.** When a read fails with `output_limit` or `io_failure`, the backend closes the session, and its 250 ms reaper emits `atlas-session-closed`. That event and the read's own error reply race to the page. If the event arrived first, the Files view cleared the session and moved to a new generation, then threw away the late reply. The user saw "Session closed" without the reason. | App | `ui/file-browser.tsx`: if the closing notice interrupts a request in flight, that request's late failure still sets the alert, unless anything has happened since. The rule is the pure function `lateFailure` in `ui/file-session-model.ts`, tested in `ui/file-session-model.test.ts` (2 tests). Protected data is still cleared at once, as before. |
| `inventory`: the Linux container did not start within 30 s | The fixture starts in 7 to 9 s when idle (4 measured starts), with each Podman call taking about a second. The suites killed the fixture 5 s after asking it to stop, but its `podman rm --force` can take longer under load. A killed fixture leaves its container running: one from the T046 runs had been up for 2 hours when found (removed). Leaked containers share the 2 GiB Podman VM and slow every later start. | Test harness | The seven Podman suites wait up to 60 s for the fixture to exit (it bounds `rm` at 30 s). Startup is allowed 90 s, as `workload-control` already did. The fixture's own per-command timeouts still bound it. |
| `desktop` line 137: Inventory in the detached Map menu not clickable for 30 s (screenshot: the panel is rolled up) | Not reproduced. A panel rolls up only on a real double-click of its title, Enter or Space on a focused title, or its Roll up button. The suite does none of these: Playwright's mouse sends `clickCount: 1`, so it cannot double-click. | Unknown: most likely outside input | None. See below. |
| `apps` line 149: final Quit click timed out (screenshot: main menu hidden, already hidden in the mid-test `apps-tab.png`) | Not reproduced. The main menu is hidden only by its toggle button or Ctrl+B, and the suite uses neither. | Unknown: most likely outside input | None. See below. |

**The two menu failures.** In both, app state changed in a way only a keyboard or mouse action can cause, and the suite performs no such action. Each suite's window opens in front and takes focus. A real keypress or click at that moment reaches the app: Ctrl+B, for example, is the sidebar shortcut in VS Code. Neither failure happened in the runs below, while the PC was left alone. This is an inference, not a proof. If either recurs with the PC untouched, look again.

**Not fully explained.** One old `files` failure screenshot (`native-files-proof-d13eac0c…`) shows a read still in flight at `/home/fixture`. Its log was overwritten, so the failing line is unknown.

## Proof

- The race in the stats write: a writer replacing a file 3000 times while a Node reader polled it failed 615 times with `PermissionError(13, 'Access is denied')`. With no reader, 0 failures in 3 × 3000 fast writes and 1200 writes at 50 ms intervals.
- The fixture fix: `files` run 5 times on the fixed binary with the **old** `ssh_fixture.py` failed once, at the "leave Files reaps the SSH child" poll (`Expected: 0, Received: 1`, timeout 15 s), with nothing in the fixture's stderr. With the fix, `inventory → files → file-changes` passed 4 of 4 times and `inventory → files` 3 of 3.
- `node --test ui/file-session-model.test.ts`: 2 of 2 pass. `pnpm check` passes.
- Full uninterrupted runs, `ATLAS_DESKTOP_BINARY=target/release/atlas-native.exe mise run test:desktop:full`, all 16 suites, release binary built from this branch, PC left alone:

| Run | Time | Result |
| --- | --- | --- |
| 1 | 14:29–14:46 | all 16 passed |
| 2 | 14:46–15:03 | all 16 passed |
| 3 | 15:03–15:20 | all 16 passed; no fixture containers left afterwards |

## Follow-ups

- `ui/protected-session.ts` (Services, Terminal and other protected views) handles `atlas-session-closed` the same way and would also drop a late failure's reason. It was not seen failing.
