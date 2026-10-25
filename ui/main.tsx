import React from "react";
import { createRoot } from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";
import { createNativeClient } from "./client";
import type { FoundationStatus } from "./contracts.gen";
import "./style.css";

function App() {
  const [status, setStatus] = React.useState<FoundationStatus | null>(null);
  const [error, setError] = React.useState("");
  React.useEffect(() => {
    if (!isTauri()) return;
    createNativeClient().status().then(setStatus).catch((e: unknown) => setError(String(e)));
  }, []);
  return <main>
    <header><span className="brand">a<span>·</span></span><span>PERSONAL WORKSPACE / ATLAS</span></header>
    <section>
      <p className="eyebrow">NATIVE FOUNDATION · F01</p>
      <h1>A little clarity.<br /><span>A stronger foundation.</span></h1>
      <p>The React and Rust application shell is being established for Windows and Android.</p>
      <div className="status" aria-live="polite">
        {error ? <><strong>Native connection failed</strong><p>{error}</p></> : status ? <>
          <strong>Rust connection verified</strong>
          <p>{status.platform} · Atlas {status.version} · {status.phase}</p>
        </> : <><strong>{isTauri() ? "Connecting to Rust…" : "Frontend preview"}</strong>
          <p>{isTauri() ? "Waiting for the native command response." : "Open the native application to verify the Rust connection."}</p></>}
      </div>
      <p className="note">This is the foundation build. Map migration and server operations are upcoming tickets. Your existing map remains in the Python reference application.</p>
    </section>
  </main>;
}

createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
