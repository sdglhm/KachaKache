type ProcessingPreviewProps = {
  progress: number;
  text: string;
};

function ProcessingPreview({ progress, text }: ProcessingPreviewProps) {
  const clamped = Math.max(0, Math.min(1, progress));
  const pct = Math.round(clamped * 100);

  return (
    <div className="mac-list-row">
      <div className="mb-2 flex items-center justify-between gap-2">
        <p className="text-[13px] font-semibold text-[var(--kk-text)]">Transcribing</p>
        <span className="mac-chip mac-chip--processing">{pct}%</span>
      </div>
      <div className="mac-progress-track">
        <div className="mac-progress-fill" style={{ width: `${pct}%` }} />
      </div>
      <p className="mt-1.5 truncate text-[12px] text-[var(--kk-text-secondary)]">{text}</p>
    </div>
  );
}

export default ProcessingPreview;
