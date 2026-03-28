import type { TriggerMode } from "../../types";

type ModeSwitchProps = {
  value: TriggerMode;
  onChange: (mode: TriggerMode) => void;
};

function ModeSwitch({ value, onChange }: ModeSwitchProps) {
  return (
    <div className="mac-segmented" role="group" aria-label="Trigger mode">
      <button
        type="button"
        onClick={() => onChange("pushToTalk")}
        className={`mac-segmented-btn ${value === "pushToTalk" ? "is-active" : ""}`}
        aria-pressed={value === "pushToTalk"}
      >
        Push to Talk
      </button>
      <button
        type="button"
        onClick={() => onChange("toggle")}
        className={`mac-segmented-btn ${value === "toggle" ? "is-active" : ""}`}
        aria-pressed={value === "toggle"}
      >
        Toggle
      </button>
    </div>
  );
}

export default ModeSwitch;
