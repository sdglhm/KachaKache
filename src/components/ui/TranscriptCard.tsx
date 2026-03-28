type TranscriptCardProps = {
  text: string;
  meta: string;
  inserted: boolean;
  copyLabel?: string;
  deleteLabel?: string;
  onCopy: () => void;
  onDelete: () => void;
};

function TranscriptCard({
  text,
  meta,
  inserted,
  copyLabel = "Copy",
  deleteLabel = "Delete",
  onCopy,
  onDelete,
}: TranscriptCardProps) {
  return (
    <article className="mac-list-row">
      <p className="mac-selectable mac-selectable--text mb-2 text-[13px] leading-relaxed text-[var(--kk-text)]">
        {text}
      </p>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-[11px] text-[var(--kk-text-tertiary)]">{meta}</span>
          <span className={`mac-chip ${inserted ? "mac-chip--success" : "mac-chip--ready"}`}>
            {inserted ? "Inserted" : "Saved"}
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          <button type="button" className="mac-btn" onClick={onCopy}>
            {copyLabel}
          </button>
          <button type="button" className="mac-btn mac-btn-destructive" onClick={onDelete}>
            {deleteLabel}
          </button>
        </div>
      </div>
    </article>
  );
}

export default TranscriptCard;
