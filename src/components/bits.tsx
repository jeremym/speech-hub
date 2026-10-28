import { useState, type ReactNode } from "react";
import { blocks } from "@overtone-foundry/app-kit";

/* The handful of app-level pieces the kit does not have. Each is kit classes and tokens only. */

/** A setting line: what it is (with a hint) on the left, the control on the right. */
export function Line({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children?: ReactNode }) {
  return (
    <div className="line">
      <div className="line-main"><strong>{label}</strong>{hint && <span className="hint">{hint}</span>}</div>
      {children && <div className="line-act">{children}</div>}
    </div>
  );
}

/** A button that asks inline before it acts: "Really remove" and "Keep it" replace it. */
export function ConfirmButton({ children, really = "Really remove", keep = "Keep it", onConfirm, className = "quiet", disabled, title }: {
  children: ReactNode; really?: string; keep?: string; onConfirm: () => void; className?: string; disabled?: boolean; title?: string;
}) {
  const [asking, setAsking] = useState(false);
  if (asking) {
    return (
      <>
        <button className="danger" onClick={() => { setAsking(false); onConfirm(); }}>{really}</button>
        <button className="quiet" onClick={() => setAsking(false)}>{keep}</button>
      </>
    );
  }
  return <button className={className} disabled={disabled} title={title} onClick={() => setAsking(true)}>{children}</button>;
}

/** A five-cell meter in the display face, for quality and speed. */
export function Meter({ value, max = 5, label }: { value: number; max?: number; label: string }) {
  const { done, rest } = blocks(value / max, max);
  return <span className="meter" role="img" aria-label={`${label} ${value} of ${max}`}>{done}<span className="rest">{rest}</span></span>;
}

/** The heading of a pane: the title, one line under it, and the pane's actions on the right. */
export function Head({ title, children, actions }: { title: ReactNode; children?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="pane-head">
      <div><h1>{title}</h1>{children && <p className="tagline">{children}</p>}</div>
      {actions && <div className="row">{actions}</div>}
    </div>
  );
}
