// Pure decisions for the Files view, kept apart from React so they can be tested.

export const fileErrors: Record<string, string> = {
  host_trust_failure: "The SSH host identity needs review. Verify it independently through your existing SSH client before retrying.",
  unauthenticated: "SSH authentication failed. Check your configured identity or unlock your existing SSH agent.",
  io_failure: "The SSH/SFTP connection failed or disconnected. Check the server and LAN/VPN connection.",
  denied: "Your SSH account cannot access this target, or its handle is no longer valid.",
  not_found: "The remote target no longer exists.",
  expired_session: "The file session has expired. Open a new session to continue.",
  output_limit: "The server response exceeded the safety limit. The session was closed without keeping partial data.",
  timed_out: "The file operation timed out. The session was closed.",
  conflict: "Another file operation is still running. Wait for it to finish.",
  remote_rejected: "The server rejected the change. Check permissions, collisions or whether the folder is empty, then inspect the refreshed result.",
  outcome_unknown: "The server may have applied this change. Open a new session and inspect the target before retrying.",
};

/**
 * Why a request failed, when its session's closing notice arrived first.
 *
 * When a read fails badly (a reply over the safety limit, a dropped
 * connection) the backend closes the session, and its reaper announces that
 * within 250 ms. The announcement and the failed request's own reply race to
 * the page. The page clears protected data on the announcement and moves to
 * a new generation, so the late reply used to be ignored and the reason lost:
 * the person saw only "Session closed". The reply still explains it when the
 * announcement interrupted that very request (`closedAt` is the generation
 * the announcement left, recorded only if a request was in flight) and
 * nothing has happened since.
 */
export function lateFailure(closedAt: number | null, epoch: number, generation: number, code: string | undefined): string | null {
  if (closedAt === null || generation !== closedAt || epoch !== closedAt - 1) return null;
  return fileErrors[code ?? ""] ?? null;
}
