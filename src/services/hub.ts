import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import type {
  HotkeyStatus, ProfileCard, ReaderText, Settings, SpeakOptions, Status, SystemInfo, VoiceInfo, WindowLabel,
} from "../types";

export const hub = {
  status: () => invoke<Status>("hub_status"),
  speak: (text: string, options?: SpeakOptions) => invoke<string>("hub_speak", { text, options }),
  open: (text: string, title?: string) => invoke<void>("hub_open", { text, title }),
  stop: () => invoke<void>("hub_stop"),
  pause: () => invoke<void>("hub_pause"),
  resume: () => invoke<void>("hub_resume"),
  togglePause: () => invoke<void>("hub_toggle_pause"),
  speakSelection: () => invoke<string>("hub_speak_selection"),
  voices: () => invoke<VoiceInfo[]>("hub_voices"),
  previewVoice: (voiceId: string) => invoke<string>("hub_preview_voice", { voiceId }),
  engines: () => invoke<ProfileCard[]>("hub_engines"),
  readerTakePending: () => invoke<ReaderText | null>("reader_take_pending"),

  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  completeOnboarding: () => invoke<Settings>("complete_onboarding"),

  axIsTrusted: () => invoke<boolean>("ax_is_trusted"),
  axRequest: () => invoke<boolean>("ax_request"),
  openSystemSettings: (pane: "accessibility" | "spoken_content" | "login_items") =>
    invoke<void>("open_system_settings", { pane }),
  hotkeysApply: () => invoke<HotkeyStatus[]>("hotkeys_apply"),

  openWindow: (label: WindowLabel, pane?: string) => invoke<void>("open_window", { label, pane }),
  hideWindow: (label: WindowLabel) => invoke<void>("hide_window", { label }),
  systemInfo: () => invoke<SystemInfo>("system_info"),
};

export interface CommandError { code: string; message: string }
export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as CommandError).message);
  return String(e);
}

/** Live hub status: initial fetch + `hub:status` events. */
export function useHubStatus(): Status | null {
  const [status, setStatus] = useState<Status | null>(null);
  useEffect(() => {
    let un: UnlistenFn | undefined;
    let alive = true;
    hub.status().then((s) => alive && setStatus(s)).catch(() => {});
    listen<Status>("hub:status", (e) => alive && setStatus(e.payload)).then((u) => (un = u));
    return () => { alive = false; un?.(); };
  }, []);
  return status;
}

/** Settings with optimistic patching. */
export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  useEffect(() => {
    hub.getSettings().then(setSettings).catch(() => {});
    const unP = listen("settings:changed", () => hub.getSettings().then(setSettings).catch(() => {}));
    return () => { unP.then((u) => u()); };
  }, []);
  const save = useCallback(async (patch: Partial<Settings>) => {
    const base = settings ?? (await hub.getSettings());
    const next = { ...base, ...patch };
    setSettings(next);
    const saved = await hub.updateSettings(next);
    setSettings(saved);
    return saved;
  }, [settings]);
  return { settings, save };
}

export function useVoices(engineId?: string): VoiceInfo[] {
  const [voices, setVoices] = useState<VoiceInfo[]>([]);
  useEffect(() => {
    hub.voices().then(setVoices).catch(() => setVoices([]));
  }, [engineId]);
  return voices;
}

export const STATE_LABEL: Record<Status["state"], string> = {
  idle: "Idle",
  loading_model: "Loading model…",
  loading_source: "Loading source…",
  ready: "Ready",
  synthesizing: "Preparing speech…",
  playing: "Speaking",
  paused: "Paused",
  cancelling: "Stopping…",
  error: "Problem",
};
