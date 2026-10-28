import { useEffect } from "react";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { Glyph } from "@overtone-foundry/app-kit";
import { hub, useHubStatus, STATE_LABEL } from "../../services/hub";

const BASE_H = 80;
const NOTICE_H = 136;

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
  const live = active && state !== "paused" && !notice;
  const dismiss = () => (active ? hub.stop() : hub.hideWindow("player"));
  return (
    <div data-tauri-drag-region className="player">
      <div data-tauri-drag-region className="player-row">
        <span className={`dot ${notice ? "bad" : "ok"}${live ? " pulse" : ""}`} aria-hidden="true" />
        <div data-tauri-drag-region className="player-main">
          <div className="player-title">{notice ? "Speech Hub" : status.title ?? "Speech Hub"}</div>
          <div className={`status-word${notice ? " red" : ""}`}>
            {notice ? "couldn't do that" : STATE_LABEL[state]}{!notice && active && status.progress.total > 1 ? ` · ${pct}%` : ""}
          </div>
        </div>
        {active && state !== "loading_source" && (
          <button className="icon" onClick={() => hub.togglePause()} aria-label={state === "paused" ? "Resume" : "Pause"} title={state === "paused" ? "Resume" : "Pause"}>
            {state === "paused" ? <Glyph name="play" /> : <span className="mark" aria-hidden="true">❚❚</span>}
          </button>
        )}
        <button className="icon" onClick={dismiss} aria-label={active ? "Stop" : "Dismiss"} title={active ? "Stop" : "Dismiss"}>
          <Glyph name={active ? "stop" : "close"} />
        </button>
      </div>
      {notice && <div className="player-notice">{notice}</div>}
    </div>
  );
}
