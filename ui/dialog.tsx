import React, { useEffect, useRef } from "react";

/** A modal dialog in InfraWerf's own style. */
export function Dialog({ title, children, close, busy = false, wide = false }: { title: string; children: React.ReactNode; close: () => void; busy?: boolean; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { const dialog = ref.current!; dialog.showModal(); return () => dialog.close(); }, []);
  return <dialog ref={ref} className={wide ? "workspace-dialog" : "form-dialog"} aria-label={title} onCancel={event => { event.preventDefault(); if (!busy) close(); }}>
    <div className="dialog-title"><div><p className="eyebrow">INFRAWERF WORKSPACE</p><h2>{title}</h2></div><button className="icon-button" aria-label="Close dialog" disabled={busy} onClick={close}>×</button></div>
    {children}
  </dialog>;
}
