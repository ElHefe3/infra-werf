import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import type { MapServer } from "./contracts.gen";
import { createWorkspaceClient } from "./workspace-client";
import { errorMessage } from "./map-model";
import "@xterm/xterm/css/xterm.css";

const client = createWorkspaceClient();
const MAX_INPUT_EVENT = 16 * 1024;
const MAX_QUEUED_INPUT = 64 * 1024;
const MAX_WRITE = 16 * 1024;

export function TerminalPanel({ server, onBusy }: { server: MapServer; onBusy: (busy: boolean) => void }) {
  const host = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const session = useRef<string | null>(null);
  const connected = useRef(false);
  const input = useRef<Uint8Array[]>([]);
  const queuedBytes = useRef(0);
  const draining = useRef(false);
  const resizeTimer = useRef<number | undefined>(undefined);
  const mounted = useRef(true);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [listenerReady, setListenerReady] = useState(false);
  const [isConnected, setIsConnected] = useState(false);
  const [message, setMessage] = useState("A protected SSH session is required.");
  const [failure, setFailure] = useState("");
  const busyCallback = useRef(onBusy);
  busyCallback.current = onBusy;

  const stop = useCallback(async (reason: string, lock = true) => {
    const id = session.current;
    session.current = null;
    connected.current = false;
    input.current = [];
    queuedBytes.current = 0;
    if (mounted.current) {
      setSessionId(null);
      setIsConnected(false);
      setConnecting(false);
      setMessage(reason);
    }
    terminal.current?.reset();
    if (id && lock) await client.lockFiles(id).catch(() => {});
    busyCallback.current(false);
  }, []);

  const pumpInput = useCallback(async () => {
    if (draining.current) return;
    draining.current = true;
    try {
      while (session.current && input.current.length) {
        const id = session.current;
        const parts: Uint8Array[] = [];
        let total = 0;
        while (input.current.length && total + input.current[0].length <= MAX_WRITE) {
          const part = input.current.shift()!;
          parts.push(part);
          total += part.length;
          queuedBytes.current -= part.length;
        }
        if (!parts.length) {
          const first = input.current.shift()!;
          const part = first.slice(0, MAX_WRITE);
          const remainder = first.slice(MAX_WRITE);
          parts.push(part);
          total = part.length;
          queuedBytes.current -= part.length;
          if (remainder.length) input.current.unshift(remainder);
        }
        const bytes = new Uint8Array(total);
        let offset = 0;
        for (const part of parts) { bytes.set(part, offset); offset += part.length; }
        await client.writeTerminal(server.id, id, bytes);
        if (session.current !== id) break;
      }
    } catch (error) {
      input.current = [];
      queuedBytes.current = 0;
      setFailure(errorMessage(error));
      await stop("The protected terminal session ended. Reconnect to continue.");
    } finally {
      draining.current = false;
      if (session.current && input.current.length) void pumpInput();
    }
  }, [server.id, stop]);

  const enqueueInput = useCallback((value: string) => {
    if (!session.current || !connected.current) return;
    if (value.length > MAX_INPUT_EVENT) {
      setFailure("This paste is too large for one terminal input. Split it into smaller pastes.");
      return;
    }
    const bytes = new TextEncoder().encode(value);
    if (bytes.length > MAX_INPUT_EVENT) {
      setFailure("This paste is too large for one terminal input. Split it into smaller pastes.");
      return;
    }
    if (queuedBytes.current + bytes.length > MAX_QUEUED_INPUT) {
      setFailure("Terminal input backed up. The session was locked to avoid sending stale keystrokes.");
      void stop("Input queue limit reached. Reconnect to continue.");
      return;
    }
    input.current.push(bytes);
    queuedBytes.current += bytes.length;
    void pumpInput();
  }, [pumpInput, stop]);

  useEffect(() => {
    mounted.current = true;
    const element = host.current;
    if (!element) return;
    const terminalInstance = new Terminal({
      cursorBlink: true,
      convertEol: false,
      scrollback: 2000,
      fontSize: 13,
      fontFamily: "Cascadia Code, Consolas, monospace",
      theme: { background: "#17251d", foreground: "#e5eee4", cursor: "#b8d58d", selectionBackground: "#385344" },
    });
    const fitAddon = new FitAddon();
    terminalInstance.loadAddon(fitAddon);
    terminalInstance.open(element);
    terminal.current = terminalInstance;
    fit.current = fitAddon;
    const inputSubscription = terminalInstance.onData(enqueueInput);
    const resize = () => {
      if (!mounted.current || !fit.current || !host.current?.clientWidth) return;
      try { fitAddon.fit(); } catch { return; }
      if (!connected.current || !session.current) return;
      window.clearTimeout(resizeTimer.current);
      resizeTimer.current = window.setTimeout(() => {
        const cols = Math.max(2, Math.min(500, terminalInstance.cols));
        const rows = Math.max(2, Math.min(300, terminalInstance.rows));
        const id = session.current;
        if (id && connected.current) void client.resizeTerminal(server.id, id, cols, rows).catch(error => {
          setFailure(errorMessage(error));
          void stop("The protected terminal session ended. Reconnect to continue.");
        });
      }, 120);
    };
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    const unlisteners: (() => void)[] = [];
    let disposed = false;
    void listen<string | null>("atlas-session-closed", event => {
      if (session.current && (event.payload === null || event.payload === session.current)) {
        void stop("Session closed or expired. Terminal contents were cleared.", false);
      }
    }).then(unlisten => {
      if (disposed) unlisten();
      else { unlisteners.push(unlisten); setListenerReady(true); }
    }).catch(error => {
      if (!disposed) setFailure(errorMessage(error));
    });
    requestAnimationFrame(resize);
    return () => {
      disposed = true;
      mounted.current = false;
      observer.disconnect();
      window.clearTimeout(resizeTimer.current);
      inputSubscription.dispose();
      for (const unlisten of unlisteners) unlisten();
      const id = session.current;
      session.current = null;
      connected.current = false;
      input.current = [];
      queuedBytes.current = 0;
      busyCallback.current(false);
      terminalInstance.reset();
      terminalInstance.dispose();
      terminal.current = null;
      fit.current = null;
      if (id) void client.lockFiles(id).catch(() => {});
    };
  }, [enqueueInput, server.id, stop]);

  useEffect(() => {
    if (!isConnected) return;
    let disposed = false;
    let polling = false;
    const poll = async () => {
      const id = session.current;
      if (!id || polling) return;
      polling = true;
      try {
        const result = await client.readTerminal(server.id, id);
        if (disposed || session.current !== id) return;
        if (result.data.length) terminal.current?.write(Uint8Array.from(result.data));
        if (result.overflow) {
          setFailure("Terminal output exceeded its memory limit. The session was locked to protect this PC.");
          await stop("Output limit reached. Reconnect to continue.");
        } else if (result.ended) {
          await stop("The remote shell ended. Reconnect to start a new session.");
        }
      } catch (error) {
        if (!disposed && session.current === id) {
          setFailure(errorMessage(error));
          await stop("The protected terminal session ended. Reconnect to continue.");
        }
      } finally {
        polling = false;
      }
    };
    const timer = window.setInterval(() => void poll(), 50);
    void poll();
    return () => { disposed = true; window.clearInterval(timer); };
  }, [isConnected, server.id, stop]);

  async function connect() {
    if (connecting || session.current || !listenerReady) return;
    setConnecting(true);
    setFailure("");
    setMessage("Authenticating with the saved SSH identity and host key…");
    busyCallback.current(true);
    let opened: string | null = null;
    try {
      const ticket = await client.prepareFiles(server.id);
      const fileSession = await client.openFiles(server.id, ticket.operation_id);
      opened = fileSession.session_id;
      session.current = opened;
      setSessionId(opened);
      fit.current?.fit();
      const cols = Math.max(2, Math.min(500, terminal.current?.cols ?? 80));
      const rows = Math.max(2, Math.min(300, terminal.current?.rows ?? 24));
      await client.startTerminal(server.id, opened, cols, rows);
      if (session.current !== opened) return;
      connected.current = true;
      setIsConnected(true);
      setConnecting(false);
      setMessage(`Connected to ${server.host}. Input and output remain in memory.`);
      terminal.current?.focus();
      busyCallback.current(false);
    } catch (error) {
      setFailure(errorMessage(error));
      await stop("Connection failed. Review the error and reconnect.");
    }
  }

  return <section className="terminal-panel" aria-label={`Terminal for ${server.name}`}>
    <header className="terminal-toolbar">
      <div><p className="eyebrow">PROTECTED SSH SESSION</p><strong>{server.name}</strong><span className="terminal-host">{server.host}</span></div>
      <div className="terminal-actions">
        <span className={`terminal-state ${isConnected ? "connected" : connecting ? "connecting" : ""}`} role="status">{isConnected ? "Connected" : connecting ? "Connecting" : "Locked"}</span>
        {isConnected ? <button className="terminal-lock" onClick={() => void stop("Session locked. Terminal contents were cleared.")}>Lock session</button>
          : <button className="primary" disabled={connecting || !listenerReady} onClick={() => void connect()}>{connecting ? "Connecting…" : listenerReady ? "Connect terminal" : "Preparing…"}</button>}
      </div>
    </header>
    <div className={`terminal-screen ${isConnected ? "active" : ""}`} ref={host} aria-label="SSH terminal" />
    <div className="terminal-message" aria-live="polite">{failure ? <span role="alert">{failure}</span> : message}</div>
    {!sessionId && <p className="terminal-help">Use the system OpenSSH identity configured on this PC. Lock the session to clear the terminal and stop the remote shell.</p>}
  </section>;
}
