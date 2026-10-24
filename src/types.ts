// Mirrors crates/speechhub-protocol (snake_case) and src-tauri/src/settings.rs (camelCase).

export type HubState =
  | "idle" | "loading_model" | "loading_source" | "ready"
  | "synthesizing" | "playing" | "paused" | "cancelling" | "error";

export interface EngineInfo {
  id: string;
  name: string;
  loaded: boolean;
  capabilities: { speed_control: boolean; voice_cloning: boolean };
}

export interface Status {
  state: HubState;
  request_id?: string;
  title?: string;
  error?: string;
  engine: EngineInfo;
  voice_id: string;
  speed: number;
  progress: { synthesized: number; total: number };
}

export interface VoiceInfo {
  id: string;
  name: string;
  language: string;
  engine_id: string;
  gender?: string;
  custom: boolean;
}

export interface SpeakOptions {
  voice_id?: string;
  speed?: number;
  title?: string;
}

export interface ProfileCard {
  id: string;
  name: string;
  tagline: string;
  sizeLabel: string;
  ramLabel: string;
  quality: number;
  speed: number;
  voiceCloning: boolean;
  bundled: boolean;
  installed: boolean;
  devOnly: boolean;
  languages: string[];
  recommended: boolean;
  license: string;
}

export interface Settings {
  onboardingComplete: boolean;
  hotkeys: { speakSelection: string; quickSpeak: string };
  engineProfile: string;
  voiceId: string;
  speed: number;
  launchAtLogin: boolean;
  showMiniPlayer: boolean;
  repeatShortcut: "replace" | "stop";
  quickSpeakAction: "palette" | "reader" | "source";
  quickSpeakSourceId: string | null;
  fallbackSourceId: string | null;
  idleUnloadMinutes: number;
}

export interface HotkeyStatus {
  id: string;
  shortcut: string;
  registered: boolean;
  error: string | null;
}

export interface SystemInfo {
  arch: string;
  memoryGb: number;
  macos: string;
  recommendedProfile: string;
  appVersion: string;
  dataDir: string;
}

export interface CliStatus {
  bundled: string | null;
  link: string;
  installed: boolean;
  inPath: boolean;
  socket: string;
}

export interface ReaderText {
  text: string;
  title: string | null;
}

export type WindowLabel = "onboarding" | "settings" | "reader" | "player" | "palette";

// ---- Sources (M4)

export type SourceKind =
  | { type: "text"; text: string }
  | { type: "file"; path: string }
  | { type: "command"; program: string; args: string[]; cwd?: string | null; shell?: boolean }
  | { type: "shortcut"; name: string; input?: string | null }
  | { type: "http"; url: string; json_path?: string | null };

export type SourceAction = "speak" | "open";

export interface Source {
  id: string;
  name: string;
  description: string;
  icon: string;
  kind: SourceKind;
  voiceId: string | null;
  speed: number | null;
  action: SourceAction;
  enabled: boolean;
  trusted: boolean;
  favorite: boolean;
  timeoutSecs: number;
  runCount: number;
  lastRunAt: number | null;
  createdAt: number;
}

export interface PublicSource {
  id: string;
  name: string;
  description: string;
  icon: string;
  type: SourceKind["type"];
  action: SourceAction;
  enabled: boolean;
  trusted: boolean;
  favorite: boolean;
  lastRunAt: number | null;
  runCount: number;
  summary: string;
}

export interface SourceTemplate {
  id: string;
  name: string;
  description: string;
  icon: string;
  kind: SourceKind;
  action: SourceAction;
  needs: string[];
}

export interface RunResult {
  sourceId: string;
  name: string;
  action: SourceAction;
  chars: number;
  fetchMs: number;
  playbackId: string | null;
}

export interface TestResult {
  preview: string;
  chars: number;
  fetchMs: number;
  truncated: boolean;
}
