# D034 — Bound displayed logs after UTF-8 decoding

Date: 2026-10-26<br>
State: verified<br>
Affected tickets: T022; preserve for later log views

## Finding

An 8 KiB byte tail can begin in the middle of a multibyte UTF-8 character. OpenSSH output is decoded lossily; replacement characters can make the resulting JavaScript string exceed the byte cap even though the original byte tail was within it. Rejecting that string turns a normal non-English log into an inspection failure.

## Evidence

The T022 native fixture emitted multibyte synthetic logs arranged so the 8 KiB tail split a character. After the repair, the adapter applies the final cap at a UTF-8 character boundary, marks the output as truncated, retains the newest tail marker, and the Windows UI journey passes. A Rust regression also simulates lossy decoding of a cut multibyte sequence.

## Consequence

Keep the remote byte tail bounded, then enforce the visible-string byte limit after decoding and trim only at character boundaries. Report trimming explicitly. The limit covers UTF-8 bytes shown to the UI, while the remote command still limits the requested log range.

## Follow-up

Reuse this rule for future live log panes and any text that is byte-bounded before UTF-8 decoding.
