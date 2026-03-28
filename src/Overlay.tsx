import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import type { DictationStatus } from "./types";

const initial: DictationStatus = { phase: "ready", message: "Ready" };
const waveformSamples = 38;

type VadStatus = {
  heardSpeech: boolean;
  speakingNow: boolean;
  silenceMs: number;
  autoStopInMs: number;
};

const defaultWave = Array.from({ length: waveformSamples }, () => 0.04);

const phaseMeta: Record<DictationStatus["phase"], { title: string; icon: string }> = {
  ready: { title: "Ready", icon: "●" },
  listening: { title: "", icon: "■" },
  processing: { title: "Transcribing", icon: "⋯" },
  done: { title: "Done", icon: "✓" },
  error: { title: "Error", icon: "!" },
};

function Overlay() {
  const [status, setStatus] = useState<DictationStatus>(initial);
  const [waveform, setWaveform] = useState<number[]>(defaultWave);
  const [, setVad] = useState<VadStatus>({
    heardSpeech: false,
    speakingNow: false,
    silenceMs: 0,
    autoStopInMs: 0,
  });

  const copy = phaseMeta[status.phase];
  const isListening = status.phase === "listening";

  const wavePath = useMemo(() => {
    const width = 124;
    const baseline = 9;
    const points = waveform.map((value, i) => {
      const x = (i / (waveform.length - 1)) * width;
      const amp = Math.pow(value, 0.8) * 8;
      const y = baseline - amp;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    return `M ${points.join(" L ")}`;
  }, [waveform]);

  useEffect(() => {
    document.documentElement.classList.add("overlay-root");
    document.body.classList.add("overlay-body");

    const unlistenHandles: UnlistenFn[] = [];

    listen<DictationStatus>("dictation://state-changed", (event) => {
      setStatus(event.payload);
      if (event.payload.phase !== "listening") {
        setWaveform(defaultWave);
      }
    }).then((unlisten) => {
      unlistenHandles.push(unlisten);
    });

    listen<number[]>("dictation://waveform", (event) => {
      const incoming = event.payload;
      if (!Array.isArray(incoming) || incoming.length === 0) return;

      setWaveform((prev) =>
        Array.from({ length: waveformSamples }, (_, i) => {
          const sourceIndex = Math.floor((i / (waveformSamples - 1)) * (incoming.length - 1));
          const raw = Math.max(0, Math.min(1, incoming[sourceIndex] ?? 0));
          const gained = Math.max(0, Math.min(1, raw * 2.4 + 0.06));
          return (prev[i] ?? 0) * 0.54 + gained * 0.46;
        }),
      );
    }).then((unlisten) => {
      unlistenHandles.push(unlisten);
    });

    listen<VadStatus>("dictation://vad", (event) => {
      setVad(event.payload);
    }).then((unlisten) => {
      unlistenHandles.push(unlisten);
    });

    return () => {
      document.documentElement.classList.remove("overlay-root");
      document.body.classList.remove("overlay-body");
      for (const cleanup of unlistenHandles) {
        cleanup();
      }
    };
  }, []);

  return (
    <main className="overlay-shell">
      <section className="overlay-card" role="status" aria-live="polite">
        {isListening ? (
          <button
            type="button"
            className={`overlay-icon overlay-stop-button ${status.phase}`}
            onClick={() => {
              void invoke("stop_dictation");
            }}
            aria-label="Stop dictation"
            title="Stop dictation"
          >
            {copy.icon}
          </button>
        ) : (
          <div className={`overlay-icon ${status.phase}`}>{copy.icon}</div>
        )}

        {isListening ? (
          <div className="overlay-wave-wrap">
            <svg viewBox="0 0 124 14" className="overlay-wave-svg" preserveAspectRatio="none" aria-hidden>
                <path d="M0 9 L124 9" stroke="rgba(124,130,144,0.34)" strokeWidth="1" fill="none" />
                <path
                  d={wavePath}
                  stroke="url(#overlayWave)"
                  strokeWidth="1.6"
                  fill="none"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <defs>
                  <linearGradient id="overlayWave" x1="0" y1="0" x2="1" y2="0">
                    <stop offset="0%" stopColor="#5d87ff" />
                    <stop offset="100%" stopColor="#7064eb" />
                  </linearGradient>
                </defs>
            </svg>
          </div>
        ) : (
          <p className="overlay-label">{copy.title}</p>
        )}
      </section>
    </main>
  );
}

export default Overlay;
