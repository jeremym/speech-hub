import { useEffect } from "react";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { hub, useHubStatus, STATE_LABEL } from "../../services/hub";

const BASE_H = 76;
const NOTICE_H = 128;

/** Floating controller shown while speaking. Never takes focus; dragging anywhere moves it.
 *  Grows to show the full text of a notice/error. */
export default function MiniPlayer() {
  const status = useHubStatus();
  const notice = status?.error ?? null;
  useEffect(() => {
    getCurrentWindow().setSize(new LogicalSize(340, notice ? NOTICE_H : BASE_H)).catch(() => {});
  }, [notice]);
  if (!status) return null;
  const { state } = status;
  const active = state === "playing" || state === "synthesizing" || state === "paused" || state === "loading_source";
  const pct = status.progress.total ? Math.round((status.progress.synthesized / status.progress.total) * 100) : 0;
  const dot = notice ? "bg-[var(--danger)]" : state === "paused" ? "bg-[var(--muted)]" : active ? "bg-[var(--ok)] animate-pulse" : "bg-[var(--muted)]";
  const dismiss = () => (active ? hub.stop() : hub.hideWindow("player"));
  return (
    <div data-tauri-drag-region className="flex h-full flex-col rounded-2xl border border-[var(--border)] bg-[var(--panel)] shadow-lg overflow-hidden">
      <div data-tauri-drag-region className="flex items-center gap-3 px-3" style={{ height: BASE_H }}>
        <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${dot}`} />
        <div data-tauri-drag-region className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-medium">{notice ? "Speech Hub" : status.title ?? "Speech Hub"}</div>
          <div className="truncate text-[11px] text-[var(--muted)]">
            {notice ? "Couldn't do that" : STATE_LABEL[state]}{!notice && active && status.progress.total > 1 ? ` · ${pct}%` : ""}
          </div>
        </div>
        {active && state !== "loading_source" && (
          <button onClick={() => hub.togglePause()} className="rounded-full p-1.5 hover:bg-[var(--panel-2)]" aria-label={state === "paused" ? "Resume" : "Pause"}>
            {state === "paused" ? "▶" : "❙❙"}
          </button>
        )}
        <button onClick={dismiss} className="rounded-full p-1.5 hover:bg-[var(--panel-2)]" aria-label={active ? "Stop" : "Dismiss"}>
          {active ? "■" : "✕"}
        </button>
      </div>
      {notice && (
        <div className="px-4 pb-3 -mt-1 text-[12px] leading-snug text-[var(--text)] select-text" style={{ wordBreak: "break-word" }}>
          {notice}
        </div>
      )}
    </div>
  );
}
