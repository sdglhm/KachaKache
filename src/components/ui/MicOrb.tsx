import type { DictationPhase } from "../../types";

type MicOrbProps = {
  phase: DictationPhase;
  onClick?: () => void;
  disabled?: boolean;
};

function MicGlyph() {
  return (
    <svg viewBox="0 0 24 24" className="h-5 w-5" fill="none" aria-hidden>
      <path
        d="M12 3a3.5 3.5 0 0 0-3.5 3.5v5a3.5 3.5 0 1 0 7 0v-5A3.5 3.5 0 0 0 12 3Z"
        stroke="currentColor"
        strokeWidth="1.9"
        strokeLinecap="round"
      />
      <path d="M6.5 11.5a5.5 5.5 0 0 0 11 0" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
      <path d="M12 17v3" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
    </svg>
  );
}

const phaseClass: Record<DictationPhase, string> = {
  ready: "ready",
  listening: "listening",
  processing: "processing",
  done: "done",
  error: "error",
};

function MicOrb({ phase, onClick, disabled }: MicOrbProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`mac-mic-orb ${phaseClass[phase]} ${disabled ? "cursor-not-allowed opacity-60" : "hover:brightness-[1.03]"}`}
      aria-label={phase === "listening" ? "Stop dictation" : "Start dictation"}
    >
      <span className="mac-mic-icon">
        {phase === "listening" ? "■" : phase === "processing" ? "⋯" : phase === "done" ? "✓" : phase === "error" ? "!" : <MicGlyph />}
      </span>
    </button>
  );
}

export default MicOrb;
