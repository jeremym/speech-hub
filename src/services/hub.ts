import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import type {
  CliStatus, CustomVoice, DownloadProgress, HotkeyStatus, ProfileCard, PublicSource, ReaderText, Rule, RulePreview,
  RunResult, Settings, Source, SourceTemplate, SpeakOptions, Status, SystemInfo, SystemSpeech, TestResult, VoiceInfo, WindowLabel,
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
  paletteSelectionPreview: () => invoke<{ preview: string; chars: number; appName: string | null } | null>("palette_selection_preview"),

  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  completeOnboarding: () => invoke<Settings>("complete_onboarding"),

  axIsTrusted: () => invoke<boolean>("ax_is_trusted"),
  axRequest: () => invoke<boolean>("ax_request"),
  openSystemSettings: (pane: "accessibility" | "spoken_content" | "login_items") =>
    invoke<void>("open_system_settings", { pane }),
  hotkeysApply: () => invoke<HotkeyStatus[]>("hotkeys_apply"),
  systemSpeech: () => invoke<SystemSpeech>("system_speech"),

  openWindow: (label: WindowLabel, pane?: string) => invoke<void>("open_window", { label, pane }),
  openWindowWithPrefill: (label: WindowLabel, pane: string, prefill: { text: string; context: string }) =>
    invoke<void>("open_window", { label, pane, prefill }),
  cliStatus: () => invoke<CliStatus>("cli_status"),
  cliInstall: () => invoke<CliStatus>("cli_install"),
  cliUninstall: () => invoke<CliStatus>("cli_uninstall"),
  hideWindow: (label: WindowLabel) => invoke<void>("hide_window", { label }),
  systemInfo: () => invoke<SystemInfo>("system_info"),

  sources: {
    list: () => invoke<Source[]>("sources_list"),
    listPublic: () => invoke<PublicSource[]>("sources_list_public"),
    get: (id: string) => invoke<Source | null>("sources_get", { id }),
    save: (source: Source, trust?: boolean) => invoke<Source>("sources_save", { source, trust }),
    delete: (id: string) => invoke<void>("sources_delete", { id }),
    duplicate: (id: string) => invoke<Source>("sources_duplicate", { id }),
    run: (reference: string) => invoke<RunResult>("sources_run", { reference }),
    test: (source: Source) => invoke<TestResult>("sources_test", { source }),
    templates: () => invoke<SourceTemplate[]>("sources_templates"),
    exportTo: (path: string) => invoke<void>("sources_export", { path }),
    importFrom: (path: string) => invoke<number>("sources_import", { path }),
    shortcuts: () => invoke<string[]>("shortcuts_list"),
    takePendingConfirm: () => invoke<PublicSource | null>("palette_take_pending_confirm"),
  },

  rules: {
    list: () => invoke<Rule[]>("rules_list"),
    save: (rule: Rule) => invoke<Rule>("rules_save", { rule }),
    delete: (id: string) => invoke<void>("rules_delete", { id }),
    preview: (text: string, draft?: Rule) => invoke<RulePreview>("rules_preview", { text, draft }),
    exportTo: (path: string) => invoke<void>("rules_export", { path }),
    importFrom: (path: string) => invoke<number>("rules_import", { path }),
  },
  unloadEngine: () => invoke<void>("hub_unload_engine"),

  plus: {
    download: () => invoke<void>("plus_download"),
    remove: () => invoke<void>("plus_remove"),
  },
  customVoices: {
    list: () => invoke<CustomVoice[]>("voices_custom_list"),
    add: (path: string, name: string, consent: boolean) => invoke<CustomVoice>("voices_custom_add", { path, name, consent }),
    addRecording: (wav: Uint8Array, name: string, consent: boolean) =>
      invoke<CustomVoice>("voices_custom_add_recording", wav, { headers: { "x-name": encodeURIComponent(name), "x-consent": consent ? "1" : "0" } }),
    delete: (id: string) => invoke<void>("voices_custom_delete", { id }),
  },
};

/** Progress of the Plus download, or null when idle. */
export function useDownloadProgress(): DownloadProgress | null {
  const [p, setP] = useState<DownloadProgress | null>(null);
  useEffect(() => {
    const un = listen<DownloadProgress>("models:download", (e) => setP(e.payload.phase === "done" ? null : e.payload));
    return () => { un.then((u) => u()); };
  }, []);
  return p;
}

export function useRules(): { rules: Rule[]; reload: () => void } {
  const [rules, setRules] = useState<Rule[]>([]);
  const reload = useCallback(() => { hub.rules.list().then(setRules).catch(() => {}); }, []);
  useEffect(() => {
    reload();
    const un = listen("pronunciation:changed", reload);
    return () => { un.then((u) => u()); };
  }, [reload]);
  return { rules, reload };
}

/** Live list of Sources (re-fetched on `sources:changed`). */
export function useSources(): { sources: Source[]; reload: () => void } {
  const [sources, setSources] = useState<Source[]>([]);
  const reload = useCallback(() => { hub.sources.list().then(setSources).catch(() => {}); }, []);
  useEffect(() => {
    reload();
    const un = listen("sources:changed", reload);
    return () => { un.then((u) => u()); };
  }, [reload]);
  return { sources, reload };
}

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
    const load = () => hub.voices().then(setVoices).catch(() => setVoices([]));
    load();
    const un = listen("voices:changed", load);
    return () => { un.then((u) => u()); };
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
