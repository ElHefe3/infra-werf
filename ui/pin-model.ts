// PIN handling that does not draw anything (F27/T065), kept apart from
// pin.tsx so the native client and unit tests can use it.
import type { NativeTransport } from "./workspace-client.ts";

let prompt: (() => Promise<boolean>) | null = null;
/** The interface registers how to ask for the PIN; `null` removes it. */
export function setPinPrompt(next: (() => Promise<boolean>) | null) { prompt = next; }

/** Wraps a transport so a `pin_required` refusal asks for the PIN once and retries. */
export function pinAware(transport: NativeTransport): NativeTransport {
  return async <T,>(command: string, args?: Record<string, unknown>) => {
    try { return await transport<T>(command, args); }
    catch (error) {
      const code = typeof error === "object" && error && "code" in error ? (error as { code: string }).code : "";
      if (code === "pin_required" && prompt && await prompt()) return transport<T>(command, args);
      throw error;
    }
  };
}

export function validPin(pin: string): boolean { return /^\d{4,12}$/.test(pin); }

/** Hides all but a short hint of a value. */
export function mask(value: string): string { return value.length <= 4 ? "••••" : `${"•".repeat(8)}${value.slice(-2)}`; }
