// The owner's InfraWerf PIN (F27/T065). The Rust core keeps the hash, the
// unlock timer and the attempt limit, and refuses protected views while
// locked. This file asks for the PIN when the core says it is needed, and
// hides sensitive details on screen until the owner unlocks.
import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { PinStatus } from "./contracts.gen";
import { mask, setPinPrompt, validPin } from "./pin-model.ts";

type PinContext = { status: PinStatus | null; reveal: () => Promise<boolean>; refresh: () => Promise<void> };
const Context = createContext<PinContext>({ status: null, reveal: async () => true, refresh: async () => {} });
export const usePin = () => useContext(Context);

const call = <T,>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args);
export const pinClient = {
  status: () => call<PinStatus>("pin_status"),
  unlock: (pin: string) => call<PinStatus>("pin_unlock", { pin }),
  lock: () => call<PinStatus>("pin_lock"),
  set: (current: string | null, pin: string) => call<PinStatus>("pin_set", { current, pin }),
  clear: (current: string) => call<PinStatus>("pin_clear", { current }),
};

function pinError(error: unknown): string {
  const code = typeof error === "object" && error && "code" in error ? (error as { code: string }).code : "";
  if (code === "unauthenticated") return "That PIN is not correct.";
  if (code === "denied") return "Too many wrong PINs. Wait a little, then try again.";
  if (code === "invalid_input") return "Use 4 to 12 digits.";
  return "The PIN could not be checked.";
}

export function PinProvider({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<PinStatus | null>(null);
  const [asking, setAsking] = useState(false);
  const [pin, setPin] = useState(""), [error, setError] = useState(""), [busy, setBusy] = useState(false);
  const waiting = useRef<((ok: boolean) => void)[]>([]);
  const dialog = useRef<HTMLDialogElement>(null);
  const refresh = useCallback(async () => { if (isTauri()) setStatus(await pinClient.status().catch(() => null)); }, []);
  useEffect(() => { void refresh(); const timer = window.setInterval(() => void refresh(), 15000); return () => window.clearInterval(timer); }, [refresh]);
  const ask = useCallback(() => new Promise<boolean>(resolve => { waiting.current.push(resolve); setPin(""); setError(""); setAsking(true); }), []);
  useEffect(() => { setPinPrompt(ask); return () => setPinPrompt(null); }, [ask]);
  useEffect(() => { const d = dialog.current; if (!d) return; if (asking && !d.open) d.showModal(); if (!asking && d.open) d.close(); }, [asking]);
  const finish = (ok: boolean) => { setAsking(false); setPin(""); const done = waiting.current.splice(0); done.forEach(resolve => resolve(ok)); };
  const reveal = useCallback(async () => {
    if (!isTauri()) return true;
    const now = await pinClient.status().catch(() => null);
    setStatus(now);
    if (!now || !now.set || now.unlocked) return true;
    return ask();
  }, [ask]);
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!validPin(pin)) { setError("Use 4 to 12 digits."); return; }
    setBusy(true);
    try { setStatus(await pinClient.unlock(pin)); finish(true); }
    catch (e) { setError(pinError(e)); setPin(""); }
    finally { setBusy(false); }
  }
  return <Context.Provider value={{ status, reveal, refresh }}>
    {children}
    <dialog ref={dialog} className="form-dialog pin-dialog" aria-label="Enter your PIN" onCancel={event => { event.preventDefault(); finish(false); }}>
      <form onSubmit={submit}>
        <div className="dialog-title"><div><p className="eyebrow">LOCKED</p><h2>Enter your PIN</h2></div></div>
        <p className="muted">Sensitive details and protected views stay hidden until you enter your InfraWerf PIN. It stays unlocked for 5 minutes.</p>
        <label>PIN<input type="password" inputMode="numeric" autoComplete="off" autoFocus maxLength={12} value={pin} onChange={e => setPin(e.target.value.replace(/\D/g, ""))} aria-label="PIN" /></label>
        {error && <p className="form-error" role="alert">{error}</p>}
        <div className="form-actions"><button type="button" onClick={() => finish(false)}>Cancel</button><button className="primary" disabled={busy || !pin}>{busy ? "Checking…" : "Unlock"}</button></div>
      </form>
    </dialog>
  </Context.Provider>;
}

/** A sensitive value: hidden while a PIN is set and InfraWerf is locked. */
export function Secret({ value, label, as = "code" }: { value: string; label: string; as?: "code" | "span" }) {
  const { status, reveal } = usePin();
  const [shown, setShown] = useState(false);
  const locked = Boolean(status?.set && !status.unlocked);
  useEffect(() => { if (locked) setShown(false); }, [locked]);
  const Tag = as;
  if (!status?.set || (shown && !locked)) return <Tag>{value}</Tag>;
  return <span className="secret"><Tag aria-label={`${label} hidden`}>{mask(value)}</Tag><button type="button" className="text-button" onClick={() => void reveal().then(ok => setShown(ok))}>Show</button></span>;
}

/** Top-bar control: locks again before the five minutes are up. */
export function LockButton() {
  const { status, refresh } = usePin();
  if (!status?.set || !status.unlocked) return null;
  return <button className="look-switch" title="Hide sensitive details and close open Files, Terminal and application windows" onClick={() => void pinClient.lock().then(refresh, refresh)}>🔒 Lock now</button>;
}

/** Settings → PIN: set, change or remove the PIN. */
export function PinSettings() {
  const { status, refresh } = usePin();
  const [current, setCurrent] = useState(""), [next, setNext] = useState(""), [repeat, setRepeat] = useState("");
  const [message, setMessage] = useState(""), [failure, setFailure] = useState(""), [busy, setBusy] = useState(false);
  const set = Boolean(status?.set);
  async function run(action: () => Promise<PinStatus>, done: string) {
    setBusy(true); setFailure(""); setMessage("");
    try { await action(); await refresh(); setMessage(done); setCurrent(""); setNext(""); setRepeat(""); }
    catch (e) { setFailure(pinError(e)); }
    finally { setBusy(false); }
  }
  function save(event: React.FormEvent) {
    event.preventDefault();
    if (!validPin(next)) { setFailure("Use 4 to 12 digits."); return; }
    if (next !== repeat) { setFailure("The two new PINs do not match."); return; }
    void run(() => pinClient.set(set ? current : null, next), set ? "PIN changed." : "PIN set. Sensitive details are now hidden until you enter it.");
  }
  const digits = (setter: (value: string) => void) => (e: React.ChangeEvent<HTMLInputElement>) => setter(e.target.value.replace(/\D/g, ""));
  return <>
    <p className="muted">{set ? "Host key fingerprints, application details, Files, Terminal, live workload details and application windows need your PIN. One entry unlocks them for 5 minutes." : "Set a PIN to hide host key fingerprints and application details, and to require it before Files, Terminal, live workload details or application windows open. InfraWerf keeps only a salted hash of it on this PC."}</p>
    <form className="pin-form" onSubmit={save}>
      {set && <label>Current PIN<input type="password" inputMode="numeric" autoComplete="off" maxLength={12} value={current} onChange={digits(setCurrent)} aria-label="Current PIN" /></label>}
      <label>{set ? "New PIN" : "PIN"}<input type="password" inputMode="numeric" autoComplete="off" maxLength={12} value={next} onChange={digits(setNext)} aria-label={set ? "New PIN" : "PIN"} /></label>
      <label>Repeat it<input type="password" inputMode="numeric" autoComplete="off" maxLength={12} value={repeat} onChange={digits(setRepeat)} aria-label="Repeat PIN" /></label>
      <div className="form-actions">{set && <button type="button" className="danger" disabled={busy || !current} onClick={() => void run(() => pinClient.clear(current), "PIN removed. Details are shown without it.")}>Remove PIN</button>}<button className="primary" disabled={busy || !next}>{set ? "Change PIN" : "Set PIN"}</button></div>
    </form>
    {failure && <p className="form-error" role="alert">{failure}</p>}
    {message && <p className="save-message" role="status">{message}</p>}
    <p className="muted">Forgot it? Quit InfraWerf and delete <code>pin.json</code> beside the workspace database; nothing else is lost.</p>
  </>;
}
