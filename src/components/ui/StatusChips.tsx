export type ChipTone = "neutral" | "ready" | "processing" | "active" | "success" | "error";

export type StatusChip = {
  id: string;
  label: string;
  tone?: ChipTone;
};

type StatusChipsProps = {
  items: StatusChip[];
  className?: string;
};

const toneClasses: Record<ChipTone, string> = {
  neutral: "",
  ready: "mac-chip--ready",
  processing: "mac-chip--processing",
  active: "mac-chip--active",
  success: "mac-chip--success",
  error: "mac-chip--error",
};

function StatusChips({ items, className = "" }: StatusChipsProps) {
  return (
    <div className={`flex flex-wrap items-center gap-1.5 ${className}`.trim()}>
      {items.map((item) => {
        const tone = item.tone ?? "neutral";
        return (
          <span key={item.id} className={`mac-chip ${toneClasses[tone]}`.trim()}>
            <span className="mac-chip-dot" aria-hidden />
            {item.label}
          </span>
        );
      })}
    </div>
  );
}

export default StatusChips;
