import type { ButtonHTMLAttributes, ReactNode } from "react";

export function Button({
  variant = "secondary", className = "", ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "ghost" | "danger" }) {
  const base = "inline-flex items-center justify-center gap-1.5 rounded-lg px-3.5 py-1.5 text-[14px] font-medium transition disabled:opacity-40 disabled:cursor-not-allowed";
  const look = {
    primary: "bg-[var(--accent)] text-[var(--accent-text)] hover:brightness-110",
    secondary: "bg-[var(--panel-2)] hover:brightness-105 border border-[var(--border)]",
    ghost: "hover:bg-[var(--panel-2)]",
    danger: "bg-[var(--danger)] text-white hover:brightness-110",
  }[variant];
  return <button className={`${base} ${look} ${className}`} {...props} />;
}

export function Card({ children, className = "", selected, onClick, disabled }: {
  children: ReactNode; className?: string; selected?: boolean; onClick?: () => void; disabled?: boolean;
}) {
  const interactive = onClick && !disabled;
  return (
    <div
      role={interactive ? "button" : undefined}
      tabIndex={interactive ? 0 : undefined}
      onClick={interactive ? onClick : undefined}
      onKeyDown={interactive ? (e) => (e.key === "Enter" || e.key === " ") && onClick?.() : undefined}
      className={`rounded-xl border bg-[var(--panel)] p-4 transition ${selected ? "border-[var(--accent)] ring-2 ring-[var(--accent)]/30" : "border-[var(--border)]"} ${interactive ? "cursor-pointer hover:border-[var(--accent)]/60" : ""} ${disabled ? "opacity-55" : ""} ${className}`}
    >
      {children}
    </div>
  );
}

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "accent" | "ok" | "warn" }) {
  const look = {
    neutral: "bg-[var(--panel-2)] text-[var(--muted)]",
    accent: "bg-[var(--accent)]/15 text-[var(--accent)]",
    ok: "bg-[var(--ok)]/15 text-[var(--ok)]",
    warn: "bg-[var(--danger)]/15 text-[var(--danger)]",
  }[tone];
  return <span className={`rounded-md px-1.5 py-0.5 text-[11px] font-semibold uppercase tracking-wide ${look}`}>{children}</span>;
}

export function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <label className="flex items-start justify-between gap-6 py-3 border-b border-[var(--border)] last:border-0">
      <span className="min-w-0">
        <span className="block font-medium">{label}</span>
        {hint && <span className="block text-[13px] text-[var(--muted)] mt-0.5">{hint}</span>}
      </span>
      <span className="shrink-0 flex items-center gap-2">{children}</span>
    </label>
  );
}

export function Toggle({ checked, onChange, disabled }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      type="button" role="switch" aria-checked={checked} disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 rounded-full transition ${checked ? "bg-[var(--accent)]" : "bg-[var(--panel-2)] border border-[var(--border)]"} disabled:opacity-40`}
    >
      <span className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition ${checked ? "left-[22px]" : "left-0.5"}`} />
    </button>
  );
}

export function Select<T extends string>({ value, onChange, options, disabled }: {
  value: T; onChange: (v: T) => void; options: { value: T; label: string }[]; disabled?: boolean;
}) {
  return (
    <select
      value={value} disabled={disabled} onChange={(e) => onChange(e.target.value as T)}
      className="rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2.5 py-1.5 text-[14px]"
    >
      {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
    </select>
  );
}

export function Dots({ value, max = 5 }: { value: number; max?: number }) {
  return (
    <span className="inline-flex gap-0.5" aria-label={`${value} of ${max}`}>
      {Array.from({ length: max }, (_, i) => (
        <span key={i} className={`h-1.5 w-1.5 rounded-full ${i < value ? "bg-[var(--accent)]" : "bg-[var(--border)]"}`} />
      ))}
    </span>
  );
}

/** A keyboard shortcut, always in the accent colour so it reads as “keys you press”. */
export function Kbd({ children, size = "sm", className = "" }: { children: ReactNode; size?: "sm" | "lg"; className?: string }) {
  const sz = size === "lg" ? "px-2.5 py-1 text-[17px]" : "px-1.5 py-0.5 text-[13px]";
  return <kbd className={`inline-flex items-center rounded-md border border-[var(--accent)]/40 bg-[var(--accent)]/12 font-mono font-semibold tracking-wide text-[var(--accent)] ${sz} ${className}`}>{children}</kbd>;
}

/** A titled settings block: what it is in the header, the controls underneath. `aside` sits in the header's right corner (a shortcut, a status badge). */
export function Section({ title, description, aside, children, className = "" }: {
  title: ReactNode; description?: ReactNode; aside?: ReactNode; children?: ReactNode; className?: string;
}) {
  return (
    <section className={`rounded-xl border border-[var(--border)] bg-[var(--panel)] overflow-hidden ${className}`}>
      <header className="flex items-start justify-between gap-4 border-b border-[var(--border)] bg-[var(--panel-2)]/50 px-4 py-3">
        <div className="min-w-0">
          <h2 className="text-[16px] font-semibold leading-tight">{title}</h2>
          {description && <p className="text-[13px] text-[var(--muted)] mt-1">{description}</p>}
        </div>
        {aside && <div className="shrink-0 flex items-center gap-2">{aside}</div>}
      </header>
      {children && <div className="px-4 py-1">{children}</div>}
    </section>
  );
}

/** Small explanatory paragraph; `tone="warn"` for conflicts and problems. */
export function Note({ children, tone = "muted", className = "" }: { children: ReactNode; tone?: "muted" | "warn"; className?: string }) {
  return <p className={`text-[13px] ${tone === "warn" ? "text-[var(--danger)]" : "text-[var(--muted)]"} ${className}`}>{children}</p>;
}

export function H1({ children }: { children: ReactNode }) {
  return <h1 className="text-[22px] font-semibold tracking-tight mb-1">{children}</h1>;
}
export function Lead({ children }: { children: ReactNode }) {
  return <p className="text-[var(--muted)] mb-5">{children}</p>;
}
