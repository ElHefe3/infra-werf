# T013 desktop protected operation/elevation work packet

Owner authorises all desktop batches; mobile remains future S09. T012 now proves configured keys and real Windows agent signing. Reuse T014/T017's native protected-session, authority, resource, lease and clearing core. Do not create another credential store or infer sudo from SSH authentication.

## Outcome and bounds

Open a deliberate authenticated operation session, review one fixed elevated action against its current host/endpoint, enter a password for that action, and prove fresh validation/safe cancellation/expiry without retaining reusable credentials. Initial T013 action is a harmless identity check; typed service/placement actions extend this in T023/T018. There is no arbitrary managed shell command IPC. Real owner-server changes remain outside fixture proof.

## Design to implement

- Existing system OpenSSH remains the transport and identity/agent resolver. Additional managed command channels must pin the already-authenticated session's endpoint/user/port **and host key before executing a command**, not compare a changed key after a mutation. Collect only public key-scan data, compare SHA256 against authenticated fingerprint, and use a private temporary known-host pin for the command lifetime. Preserve all strict/helper/forwarding overrides and owner trust/config. Fail closed for unsupported certificate/pin semantics until proved.
- Share protected session lifetime/lease; new views authenticate explicitly and clear/revoke at Lock, expiry, disconnect or unmount. Bind every single-use preview/ticket to authority, server/host/session and immutable backend action, with a bounded approval/operation deadline. Worker ownership persists through actual child cleanup.
- Fresh sudo uses a password-requiring validation with `-k -S -p <nonce> -v`, requiring that exact prompt plus successful authentication. The documented `-k` option with validation/command ignores cached credentials without updating them; never run standalone `sudo -k`/`-K`, modify sudoers or invalidate unrelated sessions. A no-prompt validation fails closed. Then execute only the approved typed action using fresh `-k` semantics in the same active flow. The password may be used only within that action's validation/execution and is immediately dropped/zeroised afterwards; it never enters argv, Store, audit, progress or raw errors.
- Keep stdout/stderr/prompt/output/time/concurrency bounded. Match prompts across chunk boundaries, prevent writes after cancel and never send a password to command input when no prompt occurred. Lost action delivery is uncertain; never automatically retry. Nonce and endpoint are not substitute authority.
- Native review/password UI names server, exact action and scope. Clear the password field at submission and every exit. Cancellation revokes the active flow; a new action requires a new review/password even while a protected session is still valid. Future Terminal remains a separate explicit interactive channel.

## Proof and stop condition

Use a disposable Linux container with actual OpenSSH/SFTP/sudo, bound to loopback only, with generated fixture identities/accounts and a synthetic password. Only its private filesystem/sudo policy may be modified. Reuse that fixture for later terminal, inventory, services and protected-placement proof. Test fresh success, wrong password despite valid global timestamp, no-prompt/NOPASSWD denial, cancel/expiry/revocation/host changes, no secret persistence/output, and actual native review/clearing. Obtain bounded independent auth review. Close T013 only after these checks; if the runtime or fresh semantics cannot be proved, record the precise blocker and continue independently eligible desktop T019 rather than weaken it.
