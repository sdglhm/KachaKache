import { invoke } from "@tauri-apps/api/core";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Menu } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useRef, useState } from "react";
import AppShell from "./components/ui/AppShell";
import BrandPill from "./components/ui/BrandPill";
import MicOrb from "./components/ui/MicOrb";
import ModeSwitch from "./components/ui/ModeSwitch";
import PrivacyBanner from "./components/ui/PrivacyBanner";
import ProcessingPreview from "./components/ui/ProcessingPreview";
import SetupSplash from "./components/ui/SetupSplash";
import SettingRow from "./components/ui/SettingRow";
import SuccessPreview from "./components/ui/SuccessPreview";
import WaveformPreview from "./components/ui/WaveformPreview";
import type {
  BootstrapState,
  DebugLogEvent,
  DebugLogLevel,
  DictationStatus,
  DownloadProgressEvent,
  FinalTextEvent,
  InstalledModel,
  MicrophoneDevice,
  PermissionKind,
  PermissionsStatus,
  RecommendedModel,
  RulesPatch,
  Settings,
  SettingsPatch,
  TranscriptEntry,
  TranscriptRetention,
  TriggerMode,
} from "./types";

const emptyStatus: DictationStatus = { phase: "ready", message: "Ready" };
const modifierCodes = new Set([
  "MetaLeft",
  "MetaRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
]);

type AppSection =
  | "dictation"
  | "transcripts"
  | "input"
  | "models"
  | "permissions"
  | "general"
  | "rules"
  | "debug";

type VadStatus = {
  heardSpeech: boolean;
  speakingNow: boolean;
  silenceMs: number;
  autoStopInMs: number;
};

type DebugLogEntry = DebugLogEvent & {
  id: string;
};

const defaultVad: VadStatus = {
  heardSpeech: false,
  speakingNow: false,
  silenceMs: 0,
  autoStopInMs: 0,
};

const defaultWaveform = Array.from({ length: 28 }, () => 0.08);
const maxDebugLogs = 250;
const transcriptRetentionOptions: Array<{ value: TranscriptRetention; label: string }> = [
  { value: "indefinite", label: "Keep indefinitely" },
  { value: "ninetyDays", label: "3 months" },
  { value: "thirtyDays", label: "1 month" },
  { value: "fourteenDays", label: "2 weeks" },
  { value: "sevenDays", label: "1 week" },
];

const sectionLabels: Record<AppSection, { title: string; subtitle: string }> = {
  dictation: { title: "Dictation", subtitle: "Local transcription into the active app" },
  transcripts: { title: "Transcripts", subtitle: "Recent local transcripts stored on this Mac" },
  input: { title: "Input", subtitle: "Shortcut, trigger mode, and microphone source" },
  models: { title: "Models", subtitle: "Manage on-device speech models" },
  permissions: { title: "Permissions", subtitle: "Review required macOS access" },
  general: { title: "General", subtitle: "Default behavior and retention preferences" },
  rules: { title: "Rules", subtitle: "Optional cleanup and spoken formatting behavior" },
  debug: { title: "Debug", subtitle: "Overlay simulation and session logs" },
};

function modifierTokens(event: React.KeyboardEvent<HTMLInputElement>): string[] {
  const tokens: string[] = [];
  if (event.metaKey) tokens.push("Cmd");
  if (event.ctrlKey) tokens.push("Ctrl");
  if (event.altKey) tokens.push("Alt");
  if (event.shiftKey) tokens.push("Shift");
  return tokens;
}

function keyToken(event: React.KeyboardEvent<HTMLInputElement>): string | null {
  const code = event.code;
  if (!code || modifierCodes.has(code)) {
    return null;
  }

  if (code.startsWith("Key")) {
    return code.slice(3).toUpperCase();
  }

  if (code.startsWith("Digit")) {
    return code.slice(5);
  }

  if (/^F\d{1,2}$/i.test(code)) {
    return code.toUpperCase();
  }

  const allowedCodes = new Set([
    "Space",
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Minus",
    "Equal",
    "Comma",
    "Period",
    "Slash",
    "Semicolon",
    "Quote",
    "Backquote",
    "Backslash",
    "BracketLeft",
    "BracketRight",
  ]);

  if (allowedCodes.has(code)) {
    return code;
  }

  if (event.key.length === 1) {
    return event.key.toUpperCase();
  }

  return code;
}

function humanSizeMb(sizeMb: number): string {
  if (sizeMb >= 1024) {
    return `${(sizeMb / 1024).toFixed(1)} GB`;
  }
  return `${sizeMb} MB`;
}

function serializeDebugValue(value: unknown): string {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  if (target.isContentEditable || target.closest("[contenteditable='true']")) {
    return true;
  }

  const editable = target.closest("input, textarea, select, [role='textbox'], [role='searchbox']");
  return Boolean(editable);
}

function isInteractiveTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return Boolean(
    target.closest(
      "button, a, summary, label, [role='button'], [role='switch'], [role='tab'], [role='menuitem']",
    ),
  );
}

function shouldStartToolbarDrag(target: EventTarget | null): boolean {
  return !isEditableTarget(target) && !isInteractiveTarget(target);
}

function statusToneClass(status: DictationStatus["phase"]): string {
  if (status === "listening") return "mac-badge--recording";
  if (status === "processing") return "mac-badge--processing";
  if (status === "done") return "mac-badge--success";
  if (status === "error") return "mac-badge--error";
  return "mac-badge--neutral";
}

function transcriptWordCount(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).length;
}

function estimatedTypingSeconds(wordCount: number): number {
  const typingWordsPerMinute = 40;
  return (wordCount / typingWordsPerMinute) * 60;
}

function formatSavedTime(seconds: number): string {
  const clamped = Math.max(0, seconds);

  if (clamped >= 3600) {
    return `${(clamped / 3600).toFixed(1)} hours`;
  }

  if (clamped >= 60) {
    return `${(clamped / 60).toFixed(1)} minutes`;
  }

  return `${Math.round(clamped)} seconds`;
}

function transcriptInsertionLabel(entry: TranscriptEntry): string {
  if (entry.inserted) {
    return "Inserted";
  }

  if (entry.insertionStrategy === "clipboardOnly") {
    return "Copied Fallback";
  }

  return "Saved";
}

type AppProps = {
  windowMode?: "main" | "setup";
};

type PracticeResult = {
  transcript: string;
  score: number;
};

const onboardingPracticePhrases = [
  "Schedule a design review for tomorrow morning.",
  "Please send the final notes to the whole team.",
  "Add a new paragraph and say this stays on my Mac.",
];

function normalizePracticeText(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function levenshteinDistance(left: string, right: string): number {
  if (left === right) return 0;
  if (!left.length) return right.length;
  if (!right.length) return left.length;

  const previous = Array.from({ length: right.length + 1 }, (_, index) => index);
  const current = new Array(right.length + 1).fill(0);

  for (let i = 1; i <= left.length; i += 1) {
    current[0] = i;
    for (let j = 1; j <= right.length; j += 1) {
      const cost = left[i - 1] === right[j - 1] ? 0 : 1;
      current[j] = Math.min(
        current[j - 1] + 1,
        previous[j] + 1,
        previous[j - 1] + cost,
      );
    }
    for (let j = 0; j <= right.length; j += 1) {
      previous[j] = current[j];
    }
  }

  return previous[right.length];
}

function practiceSimilarityScore(expected: string, heard: string): number {
  const normalizedExpected = normalizePracticeText(expected);
  const normalizedHeard = normalizePracticeText(heard);

  if (!normalizedExpected || !normalizedHeard) return 0;

  const distance = levenshteinDistance(normalizedExpected, normalizedHeard);
  const maxLength = Math.max(normalizedExpected.length, normalizedHeard.length);
  if (maxLength === 0) return 0;

  return Math.max(0, 1 - distance / maxLength);
}

function App({ windowMode = "main" }: AppProps) {
  const [currentSection, setCurrentSection] = useState<AppSection>("dictation");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<DictationStatus>(emptyStatus);
  const [permissions, setPermissions] = useState<PermissionsStatus | null>(null);
  const [recommendedModels, setRecommendedModels] = useState<RecommendedModel[]>([]);
  const [installedModels, setInstalledModels] = useState<InstalledModel[]>([]);
  const [microphones, setMicrophones] = useState<MicrophoneDevice[]>([]);
  const [history, setHistory] = useState<TranscriptEntry[]>([]);
  const [downloadProgress, setDownloadProgress] = useState<Record<string, DownloadProgressEvent>>(
    {},
  );
  const [lastFinal, setLastFinal] = useState<FinalTextEvent | null>(null);
  const [waveform, setWaveform] = useState<number[]>(defaultWaveform);
  const [vad, setVad] = useState<VadStatus>(defaultVad);
  const [isRecordingShortcut, setIsRecordingShortcut] = useState(false);
  const [shortcutDraft, setShortcutDraft] = useState<string | null>(null);
  const [showDebugTools, setShowDebugTools] = useState(import.meta.env.DEV);
  const [debugLogs, setDebugLogs] = useState<DebugLogEntry[]>([]);
  const [debugLevelFilter, setDebugLevelFilter] = useState<DebugLogLevel | "all">("all");
  const [debugScopeFilter, setDebugScopeFilter] = useState<string>("all");
  const [copyFeedback, setCopyFeedback] = useState<Record<string, "copy" | "copied" | "failed">>(
    {},
  );
  const [deleteFeedback, setDeleteFeedback] = useState<Record<string, "delete" | "deleting">>({});
  const [selectedTranscriptId, setSelectedTranscriptId] = useState<string | null>(null);
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [dismissedOnboarding, setDismissedOnboarding] = useState(false);
  const [onboardingModelId, setOnboardingModelId] = useState<string | null>(null);
  const [selectedPhraseIndex, setSelectedPhraseIndex] = useState(0);
  const [practiceResult, setPracticeResult] = useState<PracticeResult | null>(null);
  const [practiceRequestedAt, setPracticeRequestedAt] = useState<string | null>(null);
  const debugIntervalRef = useRef<number | null>(null);
  const debugLogCounterRef = useRef(0);
  const copyFeedbackTimeoutsRef = useRef<Record<string, number>>({});
  const transcriptRowRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const modelRowRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  const appendDebugLog = (level: DebugLogLevel, scope: string, message: string) => {
    const entry: DebugLogEntry = {
      id: `${Date.now()}-${debugLogCounterRef.current++}`,
      timestamp: new Date().toISOString(),
      level,
      scope,
      message,
    };

    setDebugLogs((prev) => [entry, ...prev].slice(0, maxDebugLogs));
  };

  const handleToolbarMouseDown = (event: React.MouseEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }

    if (!shouldStartToolbarDrag(event.target)) {
      return;
    }

    void getCurrentWindow()
      .startDragging()
      .catch(() => undefined);
  };

  useEffect(() => {
    const init = async () => {
      try {
        const bootstrap = await invoke<BootstrapState>("bootstrap_state");
        hydrate(bootstrap);
        const isDebug = await invoke<boolean>("is_debug_build").catch(() => import.meta.env.DEV);
        setShowDebugTools(Boolean(isDebug));
        appendDebugLog("info", "app", "bootstrap completed");
      } catch (err) {
        appendDebugLog("error", "app", `bootstrap failed: ${serializeDebugValue(err)}`);
        setError(String(err));
      } finally {
        setLoading(false);
      }
    };

    init().catch(() => setError("Failed to initialize KachaKache"));
  }, []);

  useEffect(() => {
    return () => {
      if (debugIntervalRef.current !== null) {
        window.clearInterval(debugIntervalRef.current);
      }
      Object.values(copyFeedbackTimeoutsRef.current).forEach((timeoutId) => {
        window.clearTimeout(timeoutId);
      });
    };
  }, []);

  useEffect(() => {
    const listeners: Promise<UnlistenFn>[] = [
      listen<DictationStatus>("dictation://state-changed", (event) => {
        setStatus(event.payload);
        appendDebugLog(
          event.payload.phase === "error" ? "error" : "info",
          "dictation",
          `state changed to ${event.payload.phase}: ${event.payload.message}`,
        );
        if (event.payload.phase !== "listening") {
          setWaveform(defaultWaveform);
          setVad(defaultVad);
        }
      }),
      listen<DownloadProgressEvent>("models://download-progress", async (event) => {
        setDownloadProgress((prev) => ({ ...prev, [event.payload.modelId]: event.payload }));
        if (event.payload.error) {
          appendDebugLog(
            "error",
            "models",
            `${event.payload.modelId} download failed: ${event.payload.error}`,
          );
        } else if (event.payload.done) {
          appendDebugLog("info", "models", `${event.payload.modelId} download completed`);
        }
        if (event.payload.done) {
          await refreshInstalledModels();
        }
      }),
      listen<TranscriptEntry[]>("history://updated", (event) => {
        setHistory(event.payload);
        appendDebugLog("debug", "history", `history updated (${event.payload.length} items)`);
      }),
      listen<FinalTextEvent>("dictation://final-text", (event) => {
        setLastFinal(event.payload);
        appendDebugLog(
          event.payload.inserted ? "info" : "warn",
          "dictation",
          `final text ready (${event.payload.text.length} chars, inserted=${event.payload.inserted})`,
        );
      }),
      listen<number[]>("dictation://waveform", (event) => {
        const incoming = event.payload;
        if (!Array.isArray(incoming) || incoming.length === 0) return;

        setWaveform((prev) =>
          Array.from({ length: 28 }, (_, i) => {
            const sourceIndex = Math.floor((i / 27) * (incoming.length - 1));
            const raw = Math.max(0, Math.min(1, incoming[sourceIndex] ?? 0));
            return (prev[i] ?? 0) * 0.72 + raw * 0.28;
          }),
        );
      }),
      listen<VadStatus>("dictation://vad", (event) => setVad(event.payload)),
      listen<DebugLogEvent>("debug://log", (event) => {
        setDebugLogs((prev) => {
          const next: DebugLogEntry = {
            ...event.payload,
            id: `${Date.now()}-${debugLogCounterRef.current++}`,
          };
          return [next, ...prev].slice(0, maxDebugLogs);
        });
      }),
    ];

    return () => {
      listeners.forEach((listenerPromise) => {
        listenerPromise.then((unlisten) => {
          unlisten();
        });
      });
    };
  }, []);

  useEffect(() => {
    if (history.length === 0) {
      setSelectedTranscriptId(null);
      return;
    }

    if (!selectedTranscriptId || !history.some((entry) => entry.id === selectedTranscriptId)) {
      setSelectedTranscriptId(history[0].id);
    }
  }, [history, selectedTranscriptId]);

  useEffect(() => {
    if (recommendedModels.length === 0) {
      setSelectedModelId(null);
      return;
    }

    if (
      settings?.activeModelId &&
      !selectedModelId &&
      recommendedModels.some((model) => model.id === settings.activeModelId)
    ) {
      setSelectedModelId(settings.activeModelId);
      return;
    }

    if (!selectedModelId || !recommendedModels.some((model) => model.id === selectedModelId)) {
      setSelectedModelId(recommendedModels[0].id);
    }
  }, [recommendedModels, selectedModelId, settings?.activeModelId]);

  useEffect(() => {
    if (recommendedModels.length === 0) {
      setOnboardingModelId(null);
      return;
    }

    const preferredModelId =
      settings?.activeModelId ??
      (recommendedModels.some((model) => model.id === "base.en") ? "base.en" : null) ??
      (recommendedModels.some((model) => model.id === "tiny.en") ? "tiny.en" : null) ??
      recommendedModels[0]?.id ??
      null;

    if (!onboardingModelId || !recommendedModels.some((model) => model.id === onboardingModelId)) {
      setOnboardingModelId(preferredModelId);
    }
  }, [recommendedModels, onboardingModelId, settings?.activeModelId]);

  useEffect(() => {
    if (!practiceRequestedAt || !lastFinal?.text) {
      return;
    }

    const expectedPhrase =
      onboardingPracticePhrases[selectedPhraseIndex] ?? onboardingPracticePhrases[0] ?? "";
    const nextScore = practiceSimilarityScore(expectedPhrase, lastFinal.text);
    setPracticeResult({
      transcript: lastFinal.text,
      score: nextScore,
    });
  }, [lastFinal, practiceRequestedAt, selectedPhraseIndex]);

  const activeModelName = useMemo(() => {
    if (!settings?.activeModelId) return "No model selected";
    return (
      installedModels.find((model) => model.id === settings.activeModelId)?.displayName ??
      settings.activeModelId
    );
  }, [installedModels, settings?.activeModelId]);

  const selectedTranscript = useMemo(
    () => history.find((entry) => entry.id === selectedTranscriptId) ?? null,
    [history, selectedTranscriptId],
  );
  const selectedModel = useMemo(
    () => recommendedModels.find((model) => model.id === selectedModelId) ?? null,
    [recommendedModels, selectedModelId],
  );
  const selectedInstalledModel = useMemo(
    () => installedModels.find((model) => model.id === selectedModelId) ?? null,
    [installedModels, selectedModelId],
  );
  const selectedModelProgress = selectedModelId ? downloadProgress[selectedModelId] : undefined;
  const onboardingSelectedModel = useMemo(
    () => recommendedModels.find((model) => model.id === onboardingModelId) ?? null,
    [recommendedModels, onboardingModelId],
  );
  const onboardingInstalledModel = useMemo(
    () => installedModels.find((model) => model.id === onboardingModelId) ?? null,
    [installedModels, onboardingModelId],
  );
  const onboardingModelProgress = onboardingModelId ? downloadProgress[onboardingModelId] : undefined;
  const shouldShowOnboarding = Boolean(
    settings && permissions && !dismissedOnboarding && !settings.onboardingCompleted,
  );

  const impactStats = useMemo(() => {
    const now = Date.now();
    const weekAgo = now - 7 * 24 * 60 * 60 * 1000;

    let totalSavedSeconds = 0;
    let weeklySavedSeconds = 0;
    let weeklyTranscriptCount = 0;

    for (const entry of history) {
      const wordCount = transcriptWordCount(entry.text);
      const estimatedTyping = estimatedTypingSeconds(wordCount);
      const dictationSeconds = Math.max(0, entry.durationMs / 1000);
      const savedSeconds = Math.max(0, estimatedTyping - dictationSeconds);
      totalSavedSeconds += savedSeconds;

      const createdAtMs = new Date(entry.createdAt).getTime();
      if (!Number.isNaN(createdAtMs) && createdAtMs >= weekAgo) {
        weeklySavedSeconds += savedSeconds;
        weeklyTranscriptCount += 1;
      }
    }

    return {
      totalSavedSeconds,
      weeklySavedSeconds,
      weeklyTranscriptCount,
    };
  }, [history]);

  const processingProgress = useMemo(() => {
    if (status.phase !== "processing") return 0;
    const found = status.message.match(/(\d{1,3})%/);
    if (found) {
      return Math.max(0, Math.min(1, Number(found[1]) / 100));
    }
    return 0.72;
  }, [status]);

  const listeningSubtitle = useMemo(() => {
    if (status.phase !== "listening") return status.message;

    if (!vad.heardSpeech) {
      return "Waiting for speech";
    }

    if (vad.speakingNow) {
      return "Speech detected";
    }

    const silenceSeconds = (vad.silenceMs / 1000).toFixed(1);
    const stopInSeconds = (vad.autoStopInMs / 1000).toFixed(1);
    return `Quiet ${silenceSeconds}s • auto-stop in ${stopInSeconds}s`;
  }, [status, vad]);

  const shortcutDisplay = isRecordingShortcut
    ? shortcutDraft ?? "Press keys..."
    : settings?.shortcut ?? "";
  const sectionInfo = sectionLabels[currentSection];
  const debugScopes = useMemo(
    () => Array.from(new Set(debugLogs.map((entry) => entry.scope))).sort(),
    [debugLogs],
  );
  const filteredDebugLogs = useMemo(
    () =>
      debugLogs.filter((entry) => {
        const levelMatch = debugLevelFilter === "all" || entry.level === debugLevelFilter;
        const scopeMatch = debugScopeFilter === "all" || entry.scope === debugScopeFilter;
        return levelMatch && scopeMatch;
      }),
    [debugLevelFilter, debugLogs, debugScopeFilter],
  );

  const focusTranscriptRow = (id: string) => {
    window.requestAnimationFrame(() => {
      transcriptRowRefs.current[id]?.focus();
    });
  };

  const focusModelRow = (id: string) => {
    window.requestAnimationFrame(() => {
      modelRowRefs.current[id]?.focus();
    });
  };

  const moveTranscriptSelection = (
    direction: "next" | "previous" | "first" | "last",
    shouldFocus = false,
  ) => {
    if (history.length === 0) return;

    const currentIndex = Math.max(
      0,
      history.findIndex((entry) => entry.id === selectedTranscriptId),
    );

    let nextIndex = currentIndex;
    if (direction === "next") nextIndex = Math.min(history.length - 1, currentIndex + 1);
    if (direction === "previous") nextIndex = Math.max(0, currentIndex - 1);
    if (direction === "first") nextIndex = 0;
    if (direction === "last") nextIndex = history.length - 1;

    const nextEntry = history[nextIndex];
    if (!nextEntry) return;
    setSelectedTranscriptId(nextEntry.id);
    if (shouldFocus) {
      focusTranscriptRow(nextEntry.id);
    }
  };

  const moveModelSelection = (
    direction: "next" | "previous" | "first" | "last",
    shouldFocus = false,
  ) => {
    if (recommendedModels.length === 0) return;

    const currentIndex = Math.max(
      0,
      recommendedModels.findIndex((model) => model.id === selectedModelId),
    );

    let nextIndex = currentIndex;
    if (direction === "next") nextIndex = Math.min(recommendedModels.length - 1, currentIndex + 1);
    if (direction === "previous") nextIndex = Math.max(0, currentIndex - 1);
    if (direction === "first") nextIndex = 0;
    if (direction === "last") nextIndex = recommendedModels.length - 1;

    const nextModel = recommendedModels[nextIndex];
    if (!nextModel) return;
    setSelectedModelId(nextModel.id);
    if (shouldFocus) {
      focusModelRow(nextModel.id);
    }
  };

  const runSelectedModelPrimaryAction = () => {
    if (!selectedModel) return;

    if (selectedModelProgress && !selectedModelProgress.done) {
      void cancelModel(selectedModel.id);
      return;
    }

    if (!selectedInstalledModel) {
      void downloadModel(selectedModel.id);
      return;
    }

    if (!selectedInstalledModel.isActive) {
      void setActiveModel(selectedModel.id);
    }
  };

  const handleTranscriptRowKeyDown = (
    event: React.KeyboardEvent<HTMLButtonElement>,
    transcriptId: string,
  ) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveTranscriptSelection("next", true);
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveTranscriptSelection("previous", true);
      return;
    }

    if (event.key === "Home") {
      event.preventDefault();
      moveTranscriptSelection("first", true);
      return;
    }

    if (event.key === "End") {
      event.preventDefault();
      moveTranscriptSelection("last", true);
      return;
    }

    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      void deleteHistory(transcriptId);
    }
  };

  const handleModelRowKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveModelSelection("next", true);
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveModelSelection("previous", true);
      return;
    }

    if (event.key === "Home") {
      event.preventDefault();
      moveModelSelection("first", true);
      return;
    }

    if (event.key === "End") {
      event.preventDefault();
      moveModelSelection("last", true);
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      runSelectedModelPrimaryAction();
    }
  };

  const hydrate = (bootstrap: BootstrapState) => {
    setSettings(bootstrap.settings);
    setStatus(bootstrap.status);
    setPermissions(bootstrap.permissions);
    setRecommendedModels(bootstrap.recommendedModels);
    setInstalledModels(bootstrap.installedModels);
    setMicrophones(bootstrap.microphones);
    setHistory(bootstrap.history);
  };

  const refreshInstalledModels = async () => {
    const installed = await invoke<InstalledModel[]>("list_installed_models");
    setInstalledModels(installed);
    appendDebugLog("debug", "models", `installed models refreshed (${installed.length})`);
  };

  const refreshPermissions = async () => {
    const next = await invoke<PermissionsStatus>("get_permissions_status");
    setPermissions(next);
    appendDebugLog(
      "debug",
      "permissions",
      `permissions refreshed (mic=${next.microphoneGranted}, accessibility=${next.accessibilityGranted})`,
    );
  };

  const patchSettings = async (patch: SettingsPatch) => {
    appendDebugLog("debug", "settings", `update requested ${serializeDebugValue(patch)}`);
    const updated = await invoke<Settings>("update_settings", { patch });
    setSettings(updated);
    appendDebugLog("info", "settings", "settings updated");
  };

  const patchRules = async (rulesPatch: RulesPatch) => patchSettings({ rules: rulesPatch });

  const reopenSetupWindow = async () => {
    setDismissedOnboarding(false);
    setPracticeResult(null);
    setPracticeRequestedAt(null);
    setSelectedPhraseIndex(0);
    await patchSettings({ onboardingCompleted: false });
    await invoke("open_setup_window");
  };

  const begin = async () => {
    appendDebugLog("info", "dictation", "manual start requested");
    return invoke("start_dictation", { trigger: "manual" });
  };

  const stop = async () => {
    appendDebugLog("info", "dictation", "manual stop requested");
    return invoke("stop_dictation");
  };

  const toggleDictation = async () => {
    if (status.phase === "listening") {
      await stop();
      return;
    }
    if (status.phase === "processing") return;
    await begin();
  };

  const downloadModel = async (modelId: string) => {
    appendDebugLog("info", "models", `download requested for ${modelId}`);
    return invoke("download_model", { modelId });
  };

  const cancelModel = async (modelId: string) => {
    appendDebugLog("warn", "models", `download cancelled for ${modelId}`);
    return invoke("cancel_model_download", { modelId });
  };

  const deleteModel = async (modelId: string) => {
    appendDebugLog("warn", "models", `delete requested for ${modelId}`);
    await invoke("delete_model", { modelId });
    await refreshInstalledModels();
  };

  const setActiveModel = async (modelId: string) => {
    appendDebugLog("info", "models", `set active model ${modelId}`);
    await invoke("set_active_model", { modelId });
    await refreshInstalledModels();
    setSettings((prev) => (prev ? { ...prev, activeModelId: modelId } : prev));
  };

  const requestPermission = async (kind: PermissionKind) => {
    appendDebugLog("info", "permissions", `permission request opened for ${kind}`);
    await invoke("request_permission", { kind });
    await refreshPermissions();
  };

  const openPermissionSettings = async (kind: PermissionKind) => {
    appendDebugLog("info", "permissions", `opening macOS settings for ${kind}`);
    return invoke("open_permission_settings", { kind });
  };

  const clearHistory = async () => {
    appendDebugLog("warn", "history", "clear history requested");
    Object.values(copyFeedbackTimeoutsRef.current).forEach((timeoutId) => {
      window.clearTimeout(timeoutId);
    });
    copyFeedbackTimeoutsRef.current = {};
    setCopyFeedback({});
    setDeleteFeedback({});
    return invoke("clear_history");
  };

  const copyHistory = async (id: string) => {
    appendDebugLog("info", "history", `copy requested for history entry ${id}`);
    setCopyFeedback((prev) => ({ ...prev, [id]: "copy" }));

    try {
      await invoke("copy_history_entry", { id });
      setCopyFeedback((prev) => ({ ...prev, [id]: "copied" }));
      const existing = copyFeedbackTimeoutsRef.current[id];
      if (existing) {
        window.clearTimeout(existing);
      }
      copyFeedbackTimeoutsRef.current[id] = window.setTimeout(() => {
        setCopyFeedback((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
        delete copyFeedbackTimeoutsRef.current[id];
      }, 1600);
    } catch (err) {
      appendDebugLog("error", "history", `copy failed for ${id}: ${serializeDebugValue(err)}`);
      setCopyFeedback((prev) => ({ ...prev, [id]: "failed" }));
      const existing = copyFeedbackTimeoutsRef.current[id];
      if (existing) {
        window.clearTimeout(existing);
      }
      copyFeedbackTimeoutsRef.current[id] = window.setTimeout(() => {
        setCopyFeedback((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
        delete copyFeedbackTimeoutsRef.current[id];
      }, 1800);
    }
  };

  const deleteHistory = async (id: string) => {
    appendDebugLog("warn", "history", `delete requested for history entry ${id}`);
    setDeleteFeedback((prev) => ({ ...prev, [id]: "deleting" }));
    const existing = copyFeedbackTimeoutsRef.current[id];
    if (existing) {
      window.clearTimeout(existing);
      delete copyFeedbackTimeoutsRef.current[id];
    }
    try {
      await invoke("delete_history_entry", { id });
      setCopyFeedback((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
      setDeleteFeedback((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
    } catch (err) {
      appendDebugLog("error", "history", `delete failed for ${id}: ${serializeDebugValue(err)}`);
      setDeleteFeedback((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
    }
  };

  const stopDebugSimulator = () => {
    if (debugIntervalRef.current !== null) {
      window.clearInterval(debugIntervalRef.current);
      debugIntervalRef.current = null;
    }
  };

  const emitDebugState = (phase: DictationStatus["phase"], message: string) =>
    emit("dictation://state-changed", { phase, message });

  const emitDebugQuiet = () => {
    void emit("dictation://waveform", Array.from({ length: 20 }, () => 0.03));
    void emit("dictation://vad", {
      heardSpeech: false,
      speakingNow: false,
      silenceMs: 0,
      autoStopInMs: 5000,
    });
  };

  const startDebugListening = () => {
    stopDebugSimulator();
    void emitDebugState("listening", "Listening");
    let silenceMs = 0;
    let speaking = true;
    let ticks = 0;
    debugIntervalRef.current = window.setInterval(() => {
      ticks += 1;
      if (ticks % 18 === 0) {
        speaking = !speaking;
      }
      silenceMs = speaking ? 0 : silenceMs + 180;
      const autoStopInMs = Math.max(0, 5000 - silenceMs);
      const t = Date.now() / 120;
      const debugWave = Array.from({ length: 20 }, (_, i) => {
        if (!speaking) return Math.max(0, 0.04 + Math.sin((t + i) * 0.09) * 0.01);
        const carrier = Math.sin((t + i * 0.9) * 0.35) * 0.5 + 0.5;
        const mod = Math.sin((t + i * 0.4) * 0.12) * 0.5 + 0.5;
        return Math.min(1, carrier * mod);
      });

      void emit("dictation://vad", {
        heardSpeech: true,
        speakingNow: speaking,
        silenceMs,
        autoStopInMs,
      });
      void emit("dictation://waveform", debugWave);
    }, 180);
  };

  const commitShortcut = async (shortcut: string) => {
    if (!settings) return;
    const previous = settings.shortcut;
    setSettings({ ...settings, shortcut });
    try {
      await patchSettings({ shortcut });
      setIsRecordingShortcut(false);
      setShortcutDraft(null);
    } catch (err) {
      setSettings({ ...settings, shortcut: previous });
      setShortcutDraft("Invalid shortcut");
      setError(String(err));
    }
  };

  const handleShortcutKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (!isRecordingShortcut) return;

    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      setIsRecordingShortcut(false);
      setShortcutDraft(null);
      return;
    }

    const modifiers = modifierTokens(event);
    const key = keyToken(event);

    if (!key) {
      setShortcutDraft(modifiers.length ? modifiers.join("+") : "Press keys...");
      return;
    }

    const fullShortcut = [...modifiers, key].join("+");
    setShortcutDraft(fullShortcut);
    void commitShortcut(fullShortcut);
  };

  const handleChromeKeyDownCapture = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.defaultPrevented || event.nativeEvent.isComposing) {
      return;
    }

    if (isEditableTarget(event.target)) {
      return;
    }

    if (isInteractiveTarget(event.target) && (event.key === " " || event.key === "Enter")) {
      return;
    }

    const isPlainPrintable =
      !event.metaKey &&
      !event.ctrlKey &&
      !event.altKey &&
      event.key.length === 1 &&
      !event.repeat;
    const isChromeOnlyKey = event.key === "Backspace";

    if (isPlainPrintable || isChromeOnlyKey) {
      event.preventDefault();
    }
  };

  const showTranscriptContextMenu = async (
    transcript: TranscriptEntry,
    event: React.MouseEvent<HTMLElement>,
  ) => {
    event.preventDefault();
    setSelectedTranscriptId(transcript.id);

    const menu = await Menu.new({
      items: [
        {
          id: `copy-${transcript.id}`,
          text: "Copy Transcript",
          action: () => {
            void copyHistory(transcript.id);
          },
        },
        {
          id: `delete-${transcript.id}`,
          text: "Delete Transcript",
          action: () => {
            void deleteHistory(transcript.id);
          },
        },
      ],
    });

    try {
      await menu.popup(new PhysicalPosition(event.clientX, event.clientY));
    } finally {
      window.setTimeout(() => {
        void menu.close();
      }, 250);
    }
  };

  const permissionChip = (granted: boolean) => (
    <span className={`mac-badge ${granted ? "mac-badge--success" : "mac-badge--error"}`}>
      {granted ? "Granted" : "Missing"}
    </span>
  );

  const debugLevelClass = (level: DebugLogLevel) => {
    if (level === "error") return "mac-log-badge mac-log-badge--error";
    if (level === "warn") return "mac-log-badge mac-log-badge--warn";
    if (level === "info") return "mac-log-badge mac-log-badge--info";
    return "mac-log-badge";
  };

  const toggleSwitch = (enabled: boolean, onClick: () => void) => (
    <button
      type="button"
      className={`mac-switch ${enabled ? "is-on" : ""}`}
      onClick={onClick}
      role="switch"
      aria-checked={enabled}
    >
      <span className="mac-switch-thumb" />
    </button>
  );

  const sidebarGroups: Array<{ label: string; items: Array<{ id: AppSection; label: string }> }> = [
    {
      label: "Library",
      items: [
        { id: "dictation", label: "Dictation" },
        { id: "transcripts", label: "Transcripts" },
      ],
    },
    {
      label: "Settings",
      items: [
        { id: "input", label: "Input" },
        { id: "models", label: "Models" },
        { id: "permissions", label: "Permissions" },
        { id: "general", label: "General" },
        { id: "rules", label: "Rules" },
      ],
    },
  ];

  if (showDebugTools) {
    sidebarGroups.push({ label: "Development", items: [{ id: "debug", label: "Debug" }] });
  }

  const renderToolbarActions = () => {
    if (!settings) return null;

    const toolbarSummary = (
      <div className="mac-toolbar-summary" aria-live="polite">
        <span className={`mac-toolbar-state is-${status.phase}`}>
          <span className="mac-toolbar-state-dot" aria-hidden />
          <span>{status.message || "Ready"}</span>
        </span>
        <span className="mac-toolbar-divider" aria-hidden />
        <span className="mac-toolbar-meta">{activeModelName}</span>
      </div>
    );

    if (currentSection === "dictation") {
      return (
        <>
          {toolbarSummary}
          <button
            type="button"
            className={`mac-btn ${status.phase === "listening" ? "" : "mac-btn-primary"}`}
            onClick={() => {
              void toggleDictation();
            }}
            disabled={status.phase === "processing"}
          >
            {status.phase === "listening" ? "Stop" : "Start"}
          </button>
        </>
      );
    }

    if (currentSection === "transcripts") {
      return (
        <>
          <div className="mac-toolbar-summary">
            <span className="mac-toolbar-meta">
              {history.length} transcript{history.length === 1 ? "" : "s"}
            </span>
            {selectedTranscript && (
              <>
                <span className="mac-toolbar-divider" aria-hidden />
                <span className="mac-toolbar-meta">
                  {new Date(selectedTranscript.createdAt).toLocaleDateString()}
                </span>
              </>
            )}
          </div>
          {selectedTranscript && (
            <>
              <button
                type="button"
                className="mac-btn"
                onClick={() => {
                  void copyHistory(selectedTranscript.id);
                }}
              >
                {copyFeedback[selectedTranscript.id] === "copied"
                  ? "Copied"
                  : copyFeedback[selectedTranscript.id] === "failed"
                    ? "Failed"
                    : "Copy"}
              </button>
              <button
                type="button"
                className="mac-btn mac-btn-destructive"
                onClick={() => {
                  void deleteHistory(selectedTranscript.id);
                }}
                disabled={deleteFeedback[selectedTranscript.id] === "deleting"}
              >
                {deleteFeedback[selectedTranscript.id] === "deleting" ? "Deleting..." : "Delete"}
              </button>
            </>
          )}
          <button
            type="button"
            className="mac-btn mac-btn-destructive"
            onClick={() => {
              void clearHistory();
            }}
            disabled={history.length === 0}
          >
            Clear All
          </button>
        </>
      );
    }

    if (currentSection === "models") {
      return (
        <>
          <div className="mac-toolbar-summary">
            {selectedModel ? (
              <>
                <span className="mac-toolbar-meta">{selectedModel.displayName}</span>
                <span className="mac-toolbar-divider" aria-hidden />
                <span className="mac-toolbar-meta">
                  {selectedInstalledModel
                    ? selectedInstalledModel.isActive
                      ? "Active"
                      : "Downloaded"
                    : selectedModelProgress && !selectedModelProgress.done
                      ? "Downloading"
                      : "Available"}
                </span>
              </>
            ) : (
              <span className="mac-toolbar-meta">No model selected</span>
            )}
          </div>
          {selectedModel && (
            <>
              {!selectedInstalledModel && !(selectedModelProgress && !selectedModelProgress.done) && (
                <button type="button" className="mac-btn" onClick={() => void downloadModel(selectedModel.id)}>
                  Download
                </button>
              )}
              {selectedModelProgress && !selectedModelProgress.done && (
                <button type="button" className="mac-btn" onClick={() => void cancelModel(selectedModel.id)}>
                  Cancel
                </button>
              )}
              {selectedInstalledModel && !selectedInstalledModel.isActive && (
                <button type="button" className="mac-btn" onClick={() => void setActiveModel(selectedModel.id)}>
                  Set Active
                </button>
              )}
              {selectedInstalledModel && (
                <button
                  type="button"
                  className="mac-btn mac-btn-destructive"
                  onClick={() => void deleteModel(selectedModel.id)}
                >
                  Delete
                </button>
              )}
            </>
          )}
        </>
      );
    }

    if (currentSection === "debug") {
      return (
        <button
          type="button"
          className="mac-btn"
          onClick={() => {
            setDebugLogs([]);
          }}
        >
          Clear Log
        </button>
      );
    }

    return toolbarSummary;
  };

  const renderDictationView = () => (
    <div className="mac-content-scroll">
      <section className="mac-section-card">
        <div className="mac-dictation-layout">
          <div className="mac-dictation-orb-wrap">
            <MicOrb
              phase={status.phase}
              onClick={() => {
                void toggleDictation();
              }}
              disabled={status.phase === "processing"}
            />
          </div>

          <div className="min-w-0 flex-1">
            <h1 className="mac-page-title mac-page-title--flush">
              {status.phase === "listening"
                ? "Listening"
                : status.phase === "processing"
                  ? "Transcribing"
                  : status.phase === "done"
                    ? "Ready for the next dictation"
                    : "Ready to dictate"}
            </h1>
            <p className="mac-page-subtitle">
              Audio is captured and transcribed on this Mac, then inserted into the active app.
            </p>

            <div className="mac-inline-meta">
              <span className={`mac-badge ${statusToneClass(status.phase)}`}>{status.message || "Ready"}</span>
              <span>{activeModelName}</span>
              <span>Smart Insertion</span>
            </div>
          </div>
        </div>
      </section>

      <section className="mac-group">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Audio Monitor</h2>
            <p className="mac-group-subtitle">Live microphone level and speech-end detection.</p>
          </div>
          <span className="mac-group-note">{listeningSubtitle}</span>
        </div>
        <WaveformPreview samples={waveform} calm={status.phase !== "listening"} />
      </section>

      {status.phase === "processing" && (
        <section className="mac-group">
          <ProcessingPreview
            progress={processingProgress}
            text={status.message || "Finishing local transcription..."}
          />
        </section>
      )}

      {lastFinal?.text && status.phase !== "processing" && (
        <section className="mac-group">
          <SuccessPreview text={lastFinal.text} inserted={lastFinal.inserted} />
        </section>
      )}

      <div className="mac-impact-grid">
        <article className="mac-impact-card">
          <p className="mac-impact-kicker">Since you started</p>
          <h3 className="mac-impact-value">{formatSavedTime(impactStats.totalSavedSeconds)}</h3>
          <p className="mac-impact-note">saved</p>
        </article>
        <article className="mac-impact-card">
          <p className="mac-impact-kicker">Weekly report</p>
          <h3 className="mac-impact-value">{formatSavedTime(impactStats.weeklySavedSeconds)}</h3>
          <p className="mac-impact-note">
            {impactStats.weeklyTranscriptCount > 0
              ? `${impactStats.weeklyTranscriptCount} transcript${impactStats.weeklyTranscriptCount === 1 ? "" : "s"} this week`
              : "No transcripts this week"}
          </p>
        </article>
      </div>
    </div>
  );

  const renderTranscriptsView = () => (
    <div className="mac-content-scroll mac-content-scroll--tight">
      {history.length === 0 ? (
        <section className="mac-empty-state">
          <h2 className="mac-page-title mac-page-title--compact">No transcripts yet</h2>
          <p className="mac-page-subtitle">Completed dictation stays here locally so you can review, copy, or remove anything sensitive.</p>
        </section>
      ) : (
        <section className="mac-detail-layout">
          <aside className="mac-record-list" role="listbox" aria-label="Transcripts">
            {history.map((entry) => {
              const selected = entry.id === selectedTranscriptId;
              return (
                <button
                  key={entry.id}
                  type="button"
                  className={`mac-record-row ${selected ? "is-selected" : ""}`}
                  ref={(node) => {
                    transcriptRowRefs.current[entry.id] = node;
                  }}
                  role="option"
                  aria-selected={selected}
                  tabIndex={selected ? 0 : -1}
                  onClick={() => setSelectedTranscriptId(entry.id)}
                  onFocus={() => setSelectedTranscriptId(entry.id)}
                  onKeyDown={(event) => handleTranscriptRowKeyDown(event, entry.id)}
                  onContextMenu={(event) => {
                    void showTranscriptContextMenu(entry, event);
                  }}
                >
                  <div className="mac-record-row-head">
                    <span className="mac-record-title">
                      {entry.text.split(/\n+/)[0]?.slice(0, 68) || "Untitled Transcript"}
                    </span>
                    <span className="mac-record-time">
                      {new Date(entry.createdAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
                    </span>
                  </div>
                  <p className="mac-record-snippet">{entry.text}</p>
                  <div className="mac-record-meta">
                    <span>{new Date(entry.createdAt).toLocaleDateString()}</span>
                    <span>{entry.modelId}</span>
                    <span>{Math.round(entry.durationMs / 1000)}s</span>
                    <span>{transcriptInsertionLabel(entry)}</span>
                  </div>
                </button>
              );
            })}
          </aside>

          <section
            className="mac-record-detail"
            tabIndex={selectedTranscript ? 0 : -1}
            onKeyDown={(event) => {
              if (!selectedTranscript) return;

              if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "c") {
                event.preventDefault();
                void copyHistory(selectedTranscript.id);
                return;
              }

              if (event.key === "Delete" || event.key === "Backspace") {
                event.preventDefault();
                void deleteHistory(selectedTranscript.id);
              }
            }}
            onContextMenu={(event) => {
              if (selectedTranscript) {
                void showTranscriptContextMenu(selectedTranscript, event);
              }
            }}
          >
            {selectedTranscript ? (
              <>
                <div className="mac-record-detail-head">
                  <div>
                    <h2 className="mac-page-title mac-page-title--compact">
                      {new Date(selectedTranscript.createdAt).toLocaleString()}
                    </h2>
                    <p className="mac-page-subtitle">
                      {selectedTranscript.modelId} • {Math.round(selectedTranscript.durationMs / 1000)} seconds • {selectedTranscript.text.trim().split(/\s+/).length} words
                    </p>
                  </div>
                  <span
                    className={`mac-badge ${
                      selectedTranscript.inserted
                        ? "mac-badge--success"
                        : "mac-badge--neutral"
                    }`}
                  >
                    {transcriptInsertionLabel(selectedTranscript)}
                  </span>
                </div>
                <div className="mac-record-body mac-selectable mac-selectable--text">
                  {selectedTranscript.text}
                </div>
              </>
            ) : (
              <section className="mac-empty-state">
                <h2 className="mac-page-title mac-page-title--compact">Select a transcript</h2>
                <p className="mac-page-subtitle">Choose a transcript on the left to inspect or copy it.</p>
              </section>
            )}
          </section>
        </section>
      )}
    </div>
  );

  const renderInputView = () => (
    <div className="mac-content-scroll">
      <section className="mac-group mac-group--inset">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Keyboard Shortcut</h2>
            <p className="mac-group-subtitle">Use a global shortcut to start dictation from anywhere.</p>
          </div>
        </div>
        <SettingRow
          label="Dictation Shortcut"
          description={isRecordingShortcut ? "Press a key combination" : "Click to record a new shortcut"}
          control={
            <input
              value={shortcutDisplay}
              readOnly
              onClick={() => {
                setIsRecordingShortcut(true);
                setShortcutDraft(null);
              }}
              onFocus={() => {
                setIsRecordingShortcut(true);
                setShortcutDraft(null);
              }}
              onBlur={() => {
                if (isRecordingShortcut) {
                  setIsRecordingShortcut(false);
                  setShortcutDraft(null);
                }
              }}
              onKeyDown={handleShortcutKeyDown}
              className={`mac-input w-[220px] ${isRecordingShortcut ? "border-[#7aa7ff] text-[#2f5dbc]" : ""}`}
            />
          }
        />
      </section>

      <section className="mac-group mac-group--inset">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Input Devices</h2>
            <p className="mac-group-subtitle">Select how dictation starts and which microphone it uses.</p>
          </div>
        </div>
        <SettingRow
          label="Trigger Mode"
          control={
            <ModeSwitch
              value={settings?.triggerMode ?? "toggle"}
              onChange={(mode) => {
                void patchSettings({ triggerMode: mode as TriggerMode });
              }}
            />
          }
        />
        <SettingRow
          label="Microphone"
          control={
            <select
              className="mac-select w-[260px]"
              value={settings?.micDeviceId ?? ""}
              onChange={(event) => {
                void patchSettings({ micDeviceId: event.currentTarget.value || null });
              }}
            >
              <option value="">System Default</option>
              {microphones.map((mic) => (
                <option key={mic.id} value={mic.id}>
                  {mic.name}
                  {mic.isDefault ? " (default)" : ""}
                </option>
              ))}
            </select>
          }
        />
      </section>
    </div>
  );

  const renderModelsView = () => (
    <div className="mac-content-scroll mac-content-scroll--tight">
      {recommendedModels.length === 0 ? (
        <section className="mac-empty-state">
          <h2 className="mac-page-title mac-page-title--compact">No models available</h2>
          <p className="mac-page-subtitle">Recommended local models will appear here.</p>
        </section>
      ) : (
        <section className="mac-detail-layout">
          <aside className="mac-record-list" role="listbox" aria-label="Speech models">
            {recommendedModels.map((model) => {
              const isSelected = model.id === selectedModelId;
              const local = installedModels.find((installed) => installed.id === model.id);
              const progress = downloadProgress[model.id];
              const stateLabel = local
                ? local.isActive
                  ? "Active"
                  : "Downloaded"
                : progress && !progress.done
                  ? "Downloading"
                  : model.id === "base.en"
                    ? "Recommended"
                  : "Available";

              return (
                <button
                  key={model.id}
                  type="button"
                  className={`mac-record-row ${isSelected ? "is-selected" : ""}`}
                  ref={(node) => {
                    modelRowRefs.current[model.id] = node;
                  }}
                  role="option"
                  aria-selected={isSelected}
                  tabIndex={isSelected ? 0 : -1}
                  onClick={() => setSelectedModelId(model.id)}
                  onFocus={() => setSelectedModelId(model.id)}
                  onKeyDown={handleModelRowKeyDown}
                >
                  <div className="mac-record-row-head">
                    <span className="mac-record-title">{model.displayName}</span>
                    <span className="mac-record-time">{humanSizeMb(model.sizeMb)}</span>
                  </div>
                  <p className="mac-record-snippet">
                    {model.speedNote} • {model.qualityNote}
                  </p>
                  <div className="mac-record-meta">
                    <span>{stateLabel}</span>
                    {progress && !progress.done && <span>{Math.round(progress.progress * 100)}%</span>}
                    <span>{model.id}</span>
                  </div>
                </button>
              );
            })}
          </aside>

          <section className="mac-record-detail">
            {selectedModel ? (
              <>
                <div className="mac-record-detail-head">
                  <div>
                    <h2 className="mac-page-title mac-page-title--compact">{selectedModel.displayName}</h2>
                    <p className="mac-page-subtitle">
                      {humanSizeMb(selectedModel.sizeMb)} • {selectedModel.speedNote} • {selectedModel.qualityNote}
                    </p>
                  </div>
                  <span
                    className={`mac-badge ${
                      selectedInstalledModel
                        ? selectedInstalledModel.isActive
                          ? "mac-badge--success"
                          : "mac-badge--neutral"
                        : selectedModelProgress && !selectedModelProgress.done
                          ? "mac-badge--processing"
                          : "mac-badge--neutral"
                    }`}
                  >
                    {selectedInstalledModel
                      ? selectedInstalledModel.isActive
                        ? "Active"
                        : "Downloaded"
                      : selectedModelProgress && !selectedModelProgress.done
                        ? "Downloading"
                        : "Available"}
                  </span>
                </div>

                <section className="mac-group mac-group--inset">
                  <SettingRow
                    label="Model ID"
                    control={<span className="mac-value-text">{selectedModel.id}</span>}
                  />
                  <SettingRow
                    label="Model File"
                    control={<span className="mac-value-text">{selectedModel.fileName}</span>}
                  />
                  <SettingRow
                    label="Installation"
                    control={
                      <span className="mac-value-text">
                        {selectedInstalledModel
                          ? selectedInstalledModel.isActive
                            ? "Installed and active"
                            : "Installed locally"
                          : selectedModel.id === "base.en"
                            ? "Recommended default"
                            : "Not installed"}
                      </span>
                    }
                  />
                  {selectedModel.id === "base.en" && (
                    <SettingRow
                      label="Recommendation"
                      control={<span className="mac-value-text">Best default for most Macs</span>}
                    />
                  )}
                  {selectedModel.sizeMb >= 1024 && (
                    <SettingRow
                      label="Performance"
                      control={<span className="mac-value-text">Large model; may feel slower on some Macs</span>}
                    />
                  )}
                </section>

                {(selectedModelProgress && !selectedModelProgress.done) || selectedModelProgress?.error ? (
                  <section className="mac-group">
                    <div className="mac-group-header">
                      <div>
                        <h2 className="mac-group-title">Download Progress</h2>
                        <p className="mac-group-subtitle">The model is being stored locally on this Mac.</p>
                      </div>
                      <span className="mac-group-note">
                        {Math.round((selectedModelProgress?.progress ?? 0) * 100)}%
                      </span>
                    </div>
                    <div className="px-4 py-4">
                      <div className="mac-progress-track">
                        <div
                          className="mac-progress-fill"
                          style={{ width: `${Math.round((selectedModelProgress?.progress ?? 0) * 100)}%` }}
                        />
                      </div>
                      {selectedModelProgress?.error && (
                        <p className="mt-2 text-[12px] text-[var(--kk-danger)]">{selectedModelProgress.error}</p>
                      )}
                    </div>
                  </section>
                ) : null}

                <section className="mac-group">
                  <div className="mac-group-header">
                    <div>
                      <h2 className="mac-group-title">Actions</h2>
                      <p className="mac-group-subtitle">Use Return to trigger the primary action for the selected row.</p>
                    </div>
                  </div>
                  <div className="flex flex-wrap gap-2 px-4 py-4">
                    {!selectedInstalledModel && !(selectedModelProgress && !selectedModelProgress.done) && (
                      <button type="button" className="mac-btn" onClick={() => void downloadModel(selectedModel.id)}>
                        Download
                      </button>
                    )}
                    {selectedModelProgress && !selectedModelProgress.done && (
                      <button type="button" className="mac-btn" onClick={() => void cancelModel(selectedModel.id)}>
                        Cancel Download
                      </button>
                    )}
                    {selectedInstalledModel && !selectedInstalledModel.isActive && (
                      <button type="button" className="mac-btn" onClick={() => void setActiveModel(selectedModel.id)}>
                        Set Active
                      </button>
                    )}
                    {selectedInstalledModel && (
                      <button
                        type="button"
                        className="mac-btn mac-btn-destructive"
                        onClick={() => void deleteModel(selectedModel.id)}
                      >
                        Delete Local Copy
                      </button>
                    )}
                  </div>
                </section>
              </>
            ) : (
              <section className="mac-empty-state">
                <h2 className="mac-page-title mac-page-title--compact">Select a model</h2>
                <p className="mac-page-subtitle">Choose a model on the left to manage it.</p>
              </section>
            )}
          </section>
        </section>
      )}
    </div>
  );

  const renderPermissionsView = () => (
    <div className="mac-content-scroll">
      <section className="mac-group mac-group--inset">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Required Access</h2>
            <p className="mac-group-subtitle">KachaKache needs microphone and accessibility access to work correctly.</p>
          </div>
        </div>

        <SettingRow
          label="Microphone Access"
          description="Required for local audio capture"
          statusChip={permissionChip(Boolean(permissions?.microphoneGranted))}
          control={
            <div className="flex gap-1.5">
              <button
                type="button"
                className="mac-btn"
                onClick={() => {
                  void requestPermission("microphone");
                }}
              >
                Request
              </button>
              <button
                type="button"
                className="mac-btn"
                onClick={() => {
                  void openPermissionSettings("microphone");
                }}
              >
                Open Settings
              </button>
            </div>
          }
        />

        <SettingRow
          label="Accessibility Access"
          description="Required to insert dictated text into other apps"
          statusChip={permissionChip(Boolean(permissions?.accessibilityGranted))}
          control={
            <div className="flex gap-1.5">
              <button
                type="button"
                className="mac-btn"
                onClick={() => {
                  void requestPermission("accessibility");
                }}
              >
                Request
              </button>
              <button
                type="button"
                className="mac-btn"
                onClick={() => {
                  void openPermissionSettings("accessibility");
                }}
              >
                Open Settings
              </button>
            </div>
          }
        />
      </section>
    </div>
  );

  const renderGeneralView = () => (
    <div className="mac-content-scroll">
      <section className="mac-group mac-group--inset">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Behavior</h2>
            <p className="mac-group-subtitle">Default overlay, insertion, and transcript retention settings.</p>
          </div>
        </div>

        <SettingRow
          label="Show Overlay"
          control={toggleSwitch(Boolean(settings?.overlayEnabled), () => {
            void patchSettings({ overlayEnabled: !settings?.overlayEnabled });
          })}
        />

        <SettingRow
          label="Text Insertion"
          description="KachaKache types directly first and falls back only if needed"
          control={<span className="mac-value-text">Smart automatic</span>}
        />

        <SettingRow
          label="Transcript Retention"
          description="Automatically remove older local transcripts"
          control={
            <select
              className="mac-select min-w-[190px]"
              value={settings?.transcriptRetention}
              onChange={(event) => {
                void patchSettings({
                  transcriptRetention: event.currentTarget.value as TranscriptRetention,
                });
              }}
            >
              {transcriptRetentionOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          }
        />

        <SettingRow
          label="Silence Timeout"
          description="Milliseconds before auto-stop once speech ends"
          control={
            <input
              type="number"
              min={500}
              max={15000}
              value={settings?.silenceTimeoutMs ?? 0}
              onChange={(event) => {
                void patchSettings({ silenceTimeoutMs: Number(event.currentTarget.value) });
              }}
              className="mac-input w-[120px]"
            />
          }
        />

        <SettingRow
          label="Launch at Login"
          description="Placeholder for MVP"
          control={toggleSwitch(Boolean(settings?.launchAtLoginPlaceholder), () => {
            void patchSettings({
              launchAtLoginPlaceholder: !settings?.launchAtLoginPlaceholder,
            });
          })}
        />

        <SettingRow
          label="Setup Assistant"
          description="Show the welcome and first-run setup flow again"
          control={
            <button
              type="button"
              className="mac-btn"
              onClick={() => {
                void reopenSetupWindow();
              }}
            >
              Open Setup
            </button>
          }
        />
      </section>
    </div>
  );

  const renderRulesView = () => (
    <div className="mac-content-scroll">
      <section className="mac-group mac-group--inset">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Text Cleanup</h2>
            <p className="mac-group-subtitle">Optional post-processing rules for dictated text.</p>
          </div>
        </div>
        <SettingRow
          label="Remove Repeated Filler Words"
          description='Repeated "um", "uh", and "like"'
          control={toggleSwitch(Boolean(settings?.rules.removeFillerWords), () => {
            void patchRules({ removeFillerWords: !settings?.rules.removeFillerWords });
          })}
        />
        <SettingRow
          label="Capitalize Sentence Starts"
          control={toggleSwitch(Boolean(settings?.rules.capitalizeSentenceStarts), () => {
            void patchRules({
              capitalizeSentenceStarts: !settings?.rules.capitalizeSentenceStarts,
            });
          })}
        />
        <SettingRow
          label="Convert Pauses to Punctuation"
          control={toggleSwitch(Boolean(settings?.rules.convertPausesToPunctuation), () => {
            void patchRules({
              convertPausesToPunctuation: !settings?.rules.convertPausesToPunctuation,
            });
          })}
        />
        <SettingRow
          label="Normalize Spaces"
          control={toggleSwitch(Boolean(settings?.rules.normalizeSpaces), () => {
            void patchRules({ normalizeSpaces: !settings?.rules.normalizeSpaces });
          })}
        />
        <SettingRow
          label="Smart Newline Handling"
          control={toggleSwitch(Boolean(settings?.rules.smartNewlineHandling), () => {
            void patchRules({ smartNewlineHandling: !settings?.rules.smartNewlineHandling });
          })}
        />
        <SettingRow
          label='Detect Spoken Punctuation ("comma", "full stop")'
          control={toggleSwitch(Boolean(settings?.rules.detectSpokenPunctuation), () => {
            void patchRules({
              detectSpokenPunctuation: !settings?.rules.detectSpokenPunctuation,
            });
          })}
        />
        <SettingRow
          label='Spoken Formatting ("new line", "bullet point")'
          control={toggleSwitch(Boolean(settings?.rules.spokenFormattingRules), () => {
            void patchRules({ spokenFormattingRules: !settings?.rules.spokenFormattingRules });
          })}
        />
        <SettingRow
          label='Self-Correction ("delete that", "replace X with Y")'
          control={toggleSwitch(Boolean(settings?.rules.selfCorrectionRules), () => {
            void patchRules({ selfCorrectionRules: !settings?.rules.selfCorrectionRules });
          })}
        />
      </section>
    </div>
  );

  const renderDebugView = () => (
    <div className="mac-content-scroll">
      <section className="mac-group mac-group--inset">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Overlay Simulator</h2>
            <p className="mac-group-subtitle">Preview the overlay states without starting dictation.</p>
          </div>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <button
            type="button"
            className="mac-btn"
            onClick={() => {
              appendDebugLog("debug", "debug-ui", "simulate ready");
              stopDebugSimulator();
              void emitDebugState("ready", "Ready");
              emitDebugQuiet();
            }}
          >
            Ready
          </button>
          <button
            type="button"
            className="mac-btn"
            onClick={() => {
              appendDebugLog("debug", "debug-ui", "simulate listening");
              startDebugListening();
            }}
          >
            Listening
          </button>
          <button
            type="button"
            className="mac-btn"
            onClick={() => {
              appendDebugLog("debug", "debug-ui", "simulate processing");
              stopDebugSimulator();
              void emitDebugState("processing", "Transcribing 72%");
              void emit("dictation://waveform", Array.from({ length: 20 }, () => 0.1));
              void emit("dictation://vad", {
                heardSpeech: true,
                speakingNow: false,
                silenceMs: 1200,
                autoStopInMs: 3800,
              });
            }}
          >
            Processing
          </button>
          <button
            type="button"
            className="mac-btn"
            onClick={() => {
              appendDebugLog("debug", "debug-ui", "simulate done");
              stopDebugSimulator();
              void emitDebugState("done", "Done");
              emitDebugQuiet();
            }}
          >
            Done
          </button>
          <button
            type="button"
            className="mac-btn mac-btn-destructive"
            onClick={() => {
              appendDebugLog("debug", "debug-ui", "simulate error");
              stopDebugSimulator();
              void emitDebugState("error", "Debug error");
              emitDebugQuiet();
            }}
          >
            Error
          </button>
          <button
            type="button"
            className="mac-btn"
            onClick={() => {
              appendDebugLog(
                "error",
                "insertion",
                "Simulated failed insertion: frontmost=SimulatorApp (com.example.simulator) strategy=typed inserted=false reason=AX API rejected synthetic key events",
              );
            }}
          >
            Simulate Failed Insertion
          </button>
        </div>
      </section>

      <section className="mac-group mac-group--inset">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Setup Assistant</h2>
            <p className="mac-group-subtitle">Re-open the first-run onboarding flow for visual checks.</p>
          </div>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <button
            type="button"
            className="mac-btn"
            onClick={() => {
              void reopenSetupWindow();
            }}
          >
            Reopen Setup Wizard
          </button>
        </div>
      </section>

      <section className="mac-group mac-group--inset">
        <div className="mac-group-header">
          <div>
            <h2 className="mac-group-title">Session Log</h2>
            <p className="mac-group-subtitle">Frontend and backend events captured for this run.</p>
          </div>
          <span className="mac-group-note">{filteredDebugLogs.length} entries</span>
        </div>

        <div className="mb-3 flex flex-wrap items-center gap-2 px-4">
          <select
            className="mac-select min-w-[130px]"
            value={debugLevelFilter}
            onChange={(event) => setDebugLevelFilter(event.currentTarget.value as DebugLogLevel | "all")}
          >
            <option value="all">All levels</option>
            <option value="debug">Debug</option>
            <option value="info">Info</option>
            <option value="warn">Warn</option>
            <option value="error">Error</option>
          </select>
          <select
            className="mac-select min-w-[160px]"
            value={debugScopeFilter}
            onChange={(event) => setDebugScopeFilter(event.currentTarget.value)}
          >
            <option value="all">All scopes</option>
            {debugScopes.map((scope) => (
              <option key={scope} value={scope}>
                {scope}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="mac-btn"
            onClick={() => {
              const content = filteredDebugLogs
                .map((entry) => `[${entry.level}] [${entry.scope}] ${entry.timestamp} ${entry.message}`)
                .join("\n");
              void navigator.clipboard.writeText(content);
            }}
          >
            Copy Logs
          </button>
        </div>

        <div className="mac-log-console">
          {filteredDebugLogs.length === 0 ? (
            <div className="mac-log-empty">No debug logs yet. Start dictation or use the simulator above.</div>
          ) : (
            filteredDebugLogs.map((entry) => (
              <div key={entry.id} className="mac-log-row">
                <div className="flex items-center gap-2">
                  <span className={debugLevelClass(entry.level)}>{entry.level}</span>
                  <span className="text-[11px] font-medium text-[var(--kk-text-secondary)]">{entry.scope}</span>
                  <span className="text-[11px] text-[var(--kk-text-tertiary)]">
                    {new Date(entry.timestamp).toLocaleTimeString()}
                  </span>
                </div>
                <p className="mt-1 text-[12px] text-[var(--kk-text)]">{entry.message}</p>
              </div>
            ))
          )}
        </div>
      </section>
    </div>
  );

  const renderContent = () => {
    switch (currentSection) {
      case "dictation":
        return renderDictationView();
      case "transcripts":
        return renderTranscriptsView();
      case "input":
        return renderInputView();
      case "models":
        return renderModelsView();
      case "permissions":
        return renderPermissionsView();
      case "general":
        return renderGeneralView();
      case "rules":
        return renderRulesView();
      case "debug":
        return renderDebugView();
      default:
        return null;
    }
  };

  if (loading) {
    return (
      <AppShell>
        <div className="mac-loading-state">Loading KachaKache...</div>
      </AppShell>
    );
  }

  if (error) {
    return (
      <AppShell>
        <div className="mac-loading-state mac-loading-state--error">Error: {error}</div>
      </AppShell>
    );
  }

  if (!settings || !permissions) {
    return (
      <AppShell>
        <div className="mac-loading-state">Missing app state</div>
      </AppShell>
    );
  }

  if (windowMode === "setup" && shouldShowOnboarding) {
    return (
      <AppShell variant="native">
        <SetupSplash
          permissions={permissions}
          recommendedModels={recommendedModels}
          selectedModelId={onboardingModelId}
          selectedInstalledModel={onboardingInstalledModel}
          selectedModelProgress={onboardingModelProgress}
          practicePhrases={onboardingPracticePhrases}
          selectedPhraseIndex={selectedPhraseIndex}
          practiceResult={practiceResult}
          status={status}
          onSelectModel={(modelId) => {
            setOnboardingModelId(modelId);
            setPracticeResult(null);
          }}
          onSelectPhrase={(index) => {
            setSelectedPhraseIndex(index);
            setPracticeResult(null);
          }}
          onRequestPermission={(kind) => {
            void requestPermission(kind);
          }}
          onOpenPermissionSettings={(kind) => {
            void openPermissionSettings(kind);
          }}
          onDownloadModel={(modelId) => {
            void downloadModel(modelId);
          }}
          onCancelModel={(modelId) => {
            void cancelModel(modelId);
          }}
          onSetActiveModel={(modelId) => {
            void setActiveModel(modelId);
          }}
          onTogglePractice={() => {
            const runPractice = async () => {
              if (status.phase === "listening") {
                await stop();
                return;
              }

              if (status.phase === "processing") {
                return;
              }

              if (onboardingSelectedModel && !onboardingInstalledModel?.isActive) {
                await setActiveModel(onboardingSelectedModel.id);
              }

              setPracticeRequestedAt(new Date().toISOString());
              setPracticeResult(null);
              await begin();
            };

            void runPractice();
          }}
          onFinish={() => {
            const completeSetup = async () => {
              setDismissedOnboarding(false);
              setPracticeRequestedAt(null);
              setCurrentSection("dictation");
              await patchSettings({ onboardingCompleted: true });
              await invoke("complete_setup_flow");
            };

            void completeSetup();
          }}
          onSkip={() => {
            const dismissSetup = async () => {
              setDismissedOnboarding(true);
              await invoke("dismiss_setup_flow");
            };

            void dismissSetup();
          }}
          onToolbarMouseDown={handleToolbarMouseDown}
        />
      </AppShell>
    );
  }

  if (windowMode === "setup") {
    return (
      <AppShell variant="native">
        <div className="mac-loading-state">Setup complete.</div>
      </AppShell>
    );
  }

  return (
    <AppShell variant="native">
      <div className="mac-root" onKeyDownCapture={handleChromeKeyDownCapture}>
        <header className="mac-toolbar" data-tauri-drag-region onMouseDown={handleToolbarMouseDown}>
          <div className="mac-toolbar-drag">
            <div className="min-w-0">
              <p className="mac-toolbar-title">{sectionInfo.title}</p>
              <p className="mac-toolbar-subtitle">{sectionInfo.subtitle}</p>
            </div>
          </div>

          <div className="mac-toolbar-spacer" />

          <div className="mac-toolbar-actions">{renderToolbarActions()}</div>
        </header>

        <div className="mac-main-layout">
          <aside className="mac-sidebar">
            <BrandPill />
            <div className="mt-4 space-y-4">
              {sidebarGroups.map((group) => (
                <section key={group.label}>
                  <p className="mac-sidebar-title">{group.label}</p>
                  <div className="mt-1">
                    {group.items.map((item) => (
                      <button
                        key={item.id}
                        type="button"
                        className={`mac-sidebar-item ${currentSection === item.id ? "is-selected" : ""}`}
                        onClick={() => setCurrentSection(item.id)}
                      >
                        {item.label}
                      </button>
                    ))}
                  </div>
                </section>
              ))}
            </div>
          </aside>

          <main className="mac-content-panel">{renderContent()}</main>
        </div>

        <div className="mac-footer-strip">
          <PrivacyBanner />
        </div>
      </div>
    </AppShell>
  );
}

export default App;
