import type { JobRunView, JobSchedule } from "./contracts.gen.ts";

/** Shortest schedule, in minutes (D061). */
export const MIN_SCHEDULE_MINUTES = 5;

/** A job's schedule in words. */
export function scheduleLabel(schedule: JobSchedule | null): string {
  if (!schedule) return "No schedule: runs only when you press Run";
  if (schedule.paused) return `Schedule paused after ${schedule.failures} failed runs in a row; set it again to resume`;
  const next = new Date(schedule.next_run_at).toLocaleTimeString();
  const last = schedule.last_outcome ? ` · last run ${schedule.last_outcome}` : "";
  return `Every ${schedule.every_minutes} minutes while InfraWerf is open · next at ${next}${last}`;
}

const STOP: Record<string, string> = {
  out_of_fuel: "used all its time", memory_limit: "used too much memory", stack_limit: "recursed too deeply",
  call_limit: "made too many calls", contract_violation: "broke the plugin contract", cancelled: "was cancelled",
  trapped: "crashed", invalid_module: "is not a valid module", forbidden_import: "asked for something InfraWerf does not offer",
};

/** How a run ended, in one line. */
export function describeJobRun(run: JobRunView): string {
  if (run.state === "running") return "Running…";
  if (run.state === "cancelled") return "Cancelled: its permissions changed, or you stopped it";
  const outcome = run.outcome;
  if (run.state === "done") return outcome?.exit_code === 0 ? "Finished" : `Finished with code ${outcome?.exit_code}`;
  if (outcome?.stopped) return `Failed: the job ${STOP[outcome.stopped] ?? outcome.stopped}`;
  return `Failed${outcome?.exit_code !== null && outcome?.exit_code !== undefined ? `: exit code ${outcome.exit_code}` : ""}`;
}
