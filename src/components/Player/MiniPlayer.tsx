import { hub, useHubStatus, STATE_LABEL } from "../../services/hub";

/** Floating controller shown while speaking. Never takes focus; dragging anywhere moves it. */
export default function MiniPlayer() {
  const status = useHubStatus();
  if (!status) return null;
  const { state } = status;
  const active = state === "playing" || state === "synthesizing" || state === "paused";
  const pct = status.progress.total ? Math.round((status.progress.synthesized / status.progress.total) * 100) : 0;
  const dot = state === "error" ? "bg-[var(--danger)]" : state === "paused" ? "bg-[var(--muted)]" : active ? "bg-[var(--ok)] animate-pulse" : "bg-[var(--muted)]";
  return (
    <div data-tauri-drag-region className="flex h-full items-center gap-3 rounded-2xl border border-[var(--border)] bg-[var(--panel)] px-3 shadow-lg">
      <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${dot}`} />
      <div data-tauri-drag-region className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-medium">{status.error ?? status.title ?? "Speech Hub"}</div>
        <div className="truncate text-[11px] text-[var(--muted)]">
          {STATE_LABEL[state]}{active && status.progress.total > 1 ? ` · ${pct}%` : ""}
        </div>
      </div>
      {active && (
        <button onClick={() => hub.togglePause()} className="rounded-full p-1.5 hover:bg-[var(--panel-2)]" aria-label={state === "paused" ? "Resume" : "Pause"}>
          {state === "paused" ? "▶" : "❙❙"}
        </button>
      )}
      <button onClick={() => (active ? hub.stop() : hub.hideWindow("player"))} className="rounded-full p-1.5 hover:bg-[var(--panel-2)]" aria-label={active ? "Stop" : "Dismiss"}>
        {active ? "■" : "✕"}
      </button>
    </div>
  );
}
