import type { KeyboardEvent } from "react";
import type { DictationStatus, MicrophoneDevice, TriggerMode } from "../../types";
import MicOrb from "./MicOrb";
import ModeSwitch from "./ModeSwitch";
import StatusChips, { type StatusChip } from "./StatusChips";

type DictationHeroProps = {
  status: DictationStatus;
  shortcutDisplay: string;
  shortcutHint: string;
  isRecordingShortcut: boolean;
  triggerMode: TriggerMode;
  microphones: MicrophoneDevice[];
  selectedMicId: string | null;
  chips: StatusChip[];
  onMicClick: () => void;
  onModeChange: (mode: TriggerMode) => void;
  onMicSelect: (id: string | null) => void;
  onShortcutFocus: () => void;
  onShortcutBlur: () => void;
  onShortcutKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
  showAdvancedControls?: boolean;
};

function DictationHero({
  status,
  shortcutDisplay,
  shortcutHint,
  isRecordingShortcut,
  triggerMode,
  microphones,
  selectedMicId,
  chips,
  onMicClick,
  onModeChange,
  onMicSelect,
  onShortcutFocus,
  onShortcutBlur,
  onShortcutKeyDown,
  showAdvancedControls = true,
}: DictationHeroProps) {
  const isBusy = status.phase === "listening" || status.phase === "processing";

  return (
    <section className="mac-panel">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <MicOrb phase={status.phase} onClick={onMicClick} disabled={status.phase === "processing"} />

        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <h1 className="text-[22px] font-semibold tracking-[-0.02em] text-[var(--kk-text)]">
                {status.phase === "listening"
                  ? "Listening"
                  : status.phase === "processing"
                    ? "Transcribing"
                    : "Start Dictation"}
              </h1>
              <p className="mt-1 text-[12px] text-[var(--kk-text-secondary)]">
                {isRecordingShortcut
                  ? "Press your preferred key combination"
                  : showAdvancedControls
                    ? `Press ${shortcutHint} or click the microphone`
                    : "Click the microphone to begin"}
              </p>
            </div>
            <button
              type="button"
              className={`mac-btn ${isBusy ? "" : "mac-btn-primary"}`}
              onClick={onMicClick}
              disabled={status.phase === "processing"}
            >
              {status.phase === "listening" ? "Stop" : "Start"}
            </button>
          </div>

          <StatusChips items={chips} className="mt-3" />

          {showAdvancedControls && (
            <div className="mt-3 flex flex-wrap items-center gap-2.5">
              <ModeSwitch value={triggerMode} onChange={onModeChange} />

              <label className="flex items-center gap-2 text-[12px] text-[var(--kk-text-secondary)]">
                Mic
                <select
                  className="mac-select min-w-[180px]"
                  value={selectedMicId ?? ""}
                  onChange={(event) => onMicSelect(event.currentTarget.value || null)}
                >
                  <option value="">Internal Mic</option>
                  {microphones.map((mic) => (
                    <option key={mic.id} value={mic.id}>
                      {mic.name}
                      {mic.isDefault ? " (default)" : ""}
                    </option>
                  ))}
                </select>
              </label>

              <label className="flex min-w-[220px] items-center gap-2 text-[12px] text-[var(--kk-text-secondary)]">
                Shortcut
                <input
                  value={shortcutDisplay}
                  readOnly
                  onClick={onShortcutFocus}
                  onFocus={onShortcutFocus}
                  onBlur={onShortcutBlur}
                  onKeyDown={onShortcutKeyDown}
                  className={`mac-input w-full ${isRecordingShortcut ? "border-[#7aa7ff] text-[#2f5dbc]" : ""}`}
                />
              </label>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

export default DictationHero;
