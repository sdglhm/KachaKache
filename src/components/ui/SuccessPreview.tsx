type SuccessPreviewProps = {
  text: string;
  inserted: boolean;
};

function SuccessPreview({ text, inserted }: SuccessPreviewProps) {
  return (
    <div className="mac-list-row">
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <p className="text-[13px] font-semibold text-[var(--kk-text)]">Dictation Complete</p>
        <span className={`mac-chip ${inserted ? "mac-chip--success" : "mac-chip--ready"}`}>
          {inserted ? "Inserted" : "Saved"}
        </span>
      </div>
      <p className="line-clamp-2 text-[12px] leading-relaxed text-[var(--kk-text-secondary)]">{text}</p>
    </div>
  );
}

export default SuccessPreview;
