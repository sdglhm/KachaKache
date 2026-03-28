export type TriggerMode = "pushToTalk" | "toggle";
export type InsertionMode = "automatic";
export type InsertionStrategy = "typed" | "paste" | "clipboardOnly" | "unknown";
export type TranscriptRetention =
  | "indefinite"
  | "ninetyDays"
  | "thirtyDays"
  | "fourteenDays"
  | "sevenDays";
export type DictationPhase = "ready" | "listening" | "processing" | "done" | "error";
export type PermissionKind = "microphone" | "accessibility";

export interface RulesConfig {
  removeFillerWords: boolean;
  capitalizeSentenceStarts: boolean;
  convertPausesToPunctuation: boolean;
  normalizeSpaces: boolean;
  smartNewlineHandling: boolean;
  detectSpokenPunctuation: boolean;
  spokenFormattingRules: boolean;
  selfCorrectionRules: boolean;
}

export interface RulesPatch {
  removeFillerWords?: boolean;
  capitalizeSentenceStarts?: boolean;
  convertPausesToPunctuation?: boolean;
  normalizeSpaces?: boolean;
  smartNewlineHandling?: boolean;
  detectSpokenPunctuation?: boolean;
  spokenFormattingRules?: boolean;
  selfCorrectionRules?: boolean;
}

export interface DictationStatus {
  phase: DictationPhase;
  message: string;
}

export interface Settings {
  shortcut: string;
  triggerMode: TriggerMode;
  micDeviceId: string | null;
  activeModelId: string | null;
  insertionMode: InsertionMode;
  transcriptRetention: TranscriptRetention;
  silenceTimeoutMs: number;
  overlayEnabled: boolean;
  hideDockIcon: boolean;
  launchAtLoginPlaceholder: boolean;
  onboardingCompleted: boolean;
  rules: RulesConfig;
}

export interface SettingsPatch {
  shortcut?: string;
  triggerMode?: TriggerMode;
  micDeviceId?: string | null;
  activeModelId?: string | null;
  insertionMode?: InsertionMode;
  transcriptRetention?: TranscriptRetention;
  silenceTimeoutMs?: number;
  overlayEnabled?: boolean;
  hideDockIcon?: boolean;
  launchAtLoginPlaceholder?: boolean;
  onboardingCompleted?: boolean;
  rules?: RulesPatch;
}

export interface RecommendedModel {
  id: string;
  displayName: string;
  fileName: string;
  sizeMb: number;
  speedNote: string;
  qualityNote: string;
  url: string;
  sha256: string | null;
}

export interface InstalledModel {
  id: string;
  displayName: string;
  fileName: string;
  localPath: string;
  sizeBytes: number;
  isActive: boolean;
}

export interface DownloadProgressEvent {
  modelId: string;
  receivedBytes: number;
  totalBytes: number | null;
  progress: number;
  done: boolean;
  error: string | null;
}

export interface PermissionsStatus {
  microphoneGranted: boolean;
  accessibilityGranted: boolean;
}

export interface PermissionResult {
  kind: PermissionKind;
  granted: boolean;
}

export interface MicrophoneDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export interface TranscriptEntry {
  id: string;
  text: string;
  createdAt: string;
  modelId: string;
  durationMs: number;
  inserted: boolean;
  insertionStrategy: InsertionStrategy;
}

export interface FinalTextEvent {
  text: string;
  inserted: boolean;
  insertionStrategy: InsertionStrategy;
}

export type DebugLogLevel = "debug" | "info" | "warn" | "error";

export interface DebugLogEvent {
  timestamp: string;
  level: DebugLogLevel;
  scope: string;
  message: string;
}

export interface BootstrapState {
  settings: Settings;
  status: DictationStatus;
  permissions: PermissionsStatus;
  recommendedModels: RecommendedModel[];
  installedModels: InstalledModel[];
  microphones: MicrophoneDevice[];
  history: TranscriptEntry[];
}
