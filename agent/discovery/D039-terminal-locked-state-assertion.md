# D039 — Assert the explicit locked-terminal state

Date: 2026-10-26<br>
Affected: T024

## Finding

The integrated desktop proof expected the generic text `Operation unavailable` when opening Terminal without an SSH session. The current terminal component intentionally renders the more specific `A protected SSH session is required.` The stale proof expectation failed even though the UI correctly remained locked and exposed no terminal output.

## Resolution

After checking the terminal component and actual native accessibility tree, update the T024 assertion to the current explicit locked-state message. No product behavior needed changing.

## Consequence

Integrated UI assertions should follow the current user-visible contract. Keep the locked state explicit and avoid conflating missing authentication with an unavailable operation.
